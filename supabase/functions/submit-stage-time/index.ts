import { createClient } from "npm:@supabase/supabase-js@2.49.4";

const url = Deno.env.get("SUPABASE_URL")!;
const options = { auth: { persistSession: false, autoRefreshToken: false } };
const auth = createClient(url, Deno.env.get("SUPABASE_ANON_KEY")!, options);
const db = createClient(url, Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!, options);
const reply = (status: number, body: object) => Response.json(body, {
  status, headers: { "Access-Control-Allow-Origin": "*", ...(status === 429 ? { "Retry-After": "60" } : {}) },
});

Deno.serve(async (req) => {
  if (req.method === "OPTIONS") return new Response(null, {status:204, headers:{
    "Access-Control-Allow-Origin":"*", "Access-Control-Allow-Methods":"POST, OPTIONS",
    "Access-Control-Allow-Headers":"authorization, apikey, content-type"
  }});
  if (req.method !== "POST") return reply(405, { error: "post_required" });
  const token = req.headers.get("authorization")?.match(/^Bearer ([^\s]+)$/i)?.[1];
  if (!token) return reply(401, { error: "authentication_required" });
  const { data, error } = await auth.auth.getUser(token);
  if (error || !data.user) return reply(401, { error: "invalid_session" });
  const reader = req.body?.getReader();
  if (!reader) return reply(400, { error: "empty_body" });
  const bytes = new Uint8Array(8192);
  let size = 0;
  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    if (value.length > bytes.length - size) {
      await reader.cancel();
      return reply(413, { error: "body_too_large" });
    }
    bytes.set(value, size);
    size += value.length;
  }
  let run;
  try { run = JSON.parse(new TextDecoder().decode(bytes.subarray(0, size))); }
  catch { return reply(400, { error: "invalid_json" }); }
  if (!run || typeof run !== "object" || Array.isArray(run) ||
      "player_id" in run || "verified" in run || run.finished !== true ||
      run.finish_crossed_forward !== true || !Number.isSafeInteger(run.sim_steps) || run.sim_steps <= 0 ||
      typeof run.entry_id !== "string" || !/^lb1-[0-9a-f]{20}$/.test(run.entry_id) ||
      ![run.stage_id, run.vehicle_id].every(v => typeof v === "string" && /^[a-z0-9_-]{1,128}$/.test(v)) ||
      ![run.driving_ms, run.penalty_ms].every(v => Number.isSafeInteger(v) && v >= 0 && v <= 86400000) ||
      run.driving_ms === 0 || run.total_ms !== run.driving_ms + run.penalty_ms ||
      !Array.isArray(run.splits_ms) || run.splits_ms.length > 128 ||
      !run.splits_ms.every((v: number, i: number) => Number.isInteger(v) && v > 0 &&
        v < run.driving_ms && (i === 0 || v > run.splits_ms[i-1]))) {
    return reply(422, { error: "invalid_result" });
  }
  const { data: result, error: failed } = await db.rpc("leaderboard_submit", {
    p_uid: data.user.id, p_submission: run.entry_id, p_stage: run.stage_id,
    p_vehicle: run.vehicle_id, p_time: run.driving_ms, p_penalty: run.penalty_ms, p_splits: run.splits_ms,
  });
  if (failed || !result) return reply(503, { error: "submission_unavailable" });
  return reply(result.status, result);
});
