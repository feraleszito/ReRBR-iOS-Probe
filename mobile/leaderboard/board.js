"use strict";
const el = id => document.getElementById(id);
async function bridge(op, value = "") {
  if (window.RbrBoard) return window.RbrBoard.call(op, value);
  return window.webkit.messageHandlers.board.postMessage({op, value});
}
let session, stage, vehicle, filtered = false, offset = 0, total = 0, generation = 0;
let busy = false, active = false, loading = false, refreshQueued = false, authentication;
async function post(path, body, access) {
  const headers = {"Content-Type":"application/json", apikey:apiKey};
  if (access) headers.Authorization = "Bearer " + access;
  const response = await fetch(project + path, {method:"POST", headers, body:JSON.stringify(body),
    signal:AbortSignal.timeout(18000)});
  if (!response.ok) throw Object.assign(new Error("HTTP " + response.status), {status:response.status});
  return response.json();
}
async function refreshSession() {
  if (!session) session = JSON.parse(await bridge("session") || "null");
  if (session && session.expires > Date.now() + 60000) return;
  const data = session ? await post("/auth/v1/token?grant_type=refresh_token",{refresh_token:session.refresh})
    : await post("/auth/v1/signup", {});
  const next = {access:data.access_token, refresh:data.refresh_token, expires:Date.now()+1000*data.expires_in};
  if (!next.access || !next.refresh || await bridge("persist", JSON.stringify(next)) !== "ok")
    throw new Error("No se pudo guardar la sesión segura");
  session = next;
}
function authenticate() {
  if (!authentication) authentication = refreshSession().finally(()=>{authentication=undefined;});
  return authentication;
}
const rpc = (name, body) => post("/rest/v1/rpc/"+name, body, session.access);
function time(ms) {
  return Math.floor(ms/60000)+":"+String(Math.floor(ms/1000)%60).padStart(2,"0")+"."+String(ms%1000).padStart(3,"0");
}
async function page(reset = false) {
  if (reset) { generation++; offset=0; total=Infinity; el("rows").replaceChildren(); }
  if ((!reset && loading) || offset >= total || !session || !stage) return;
  const gen = generation;
  loading = true;
  try {
    const data = await rpc("leaderboard_page",{p_stage:stage,p_vehicle:filtered ? vehicle : null,p_offset:offset});
    if (gen !== generation) return;
    for (const r of data.rows) {
      const li = document.createElement("li");
      li.textContent = "#"+r.position+"  "+r.display_name+" · "+time(r.total_ms)+" · "+r.vehicle_id+" · "+(r.verified ? "Verificado" : "Provisional");
      el("rows").appendChild(li);
    }
    offset += data.rows.length; total = data.count;
    el("mine").textContent = data.mine ? "Mi mejor tiempo · #"+data.mine.position+" · "+time(data.mine.total_ms) : "Aún sin tiempo personal en este tramo";
    el("more").textContent = offset < total ? "Cargando más al desplazar…" : "";
  } finally { if (gen === generation) loading=false; }
}
async function update(force = false) {
  if (busy) { refreshQueued ||= force; return; }
  busy=true;
  try {
    const settings = JSON.parse(await bridge("selection") || "{}");
    const changed = stage !== settings.stage_id || vehicle !== settings.vehicle_id;
    stage=settings.stage_id; vehicle=settings.vehicle_id;
    el("stage").textContent = stage || "";
    const pending = JSON.parse(await bridge("pending") || "[]");
    if (!active && !pending.length) return;
    el("status").textContent="Conectando…";
    await authenticate();
    const name = await rpc("leaderboard_profile",{});
    if (document.activeElement !== el("pilot")) el("pilot").value=name;
    let sent=0, rejected=0, limited=false;
    for (const run of pending) {
      if(sent >= 6) break;
      try {
        const result = await post("/functions/v1/submit-stage-time",run,session.access);
        if (result.id && await bridge("ack",run.entry_id) === "ok") sent++;
      } catch (e) {
        if (e.status === 429) { limited=true; break; }
        if (e.status !== 422 && e.status !== 409) throw e;
        rejected++;
      }
    }
    if (active && (force || changed || sent || !offset)) await page(true);
    el("status").textContent = limited ? "Límite temporal · reintento automático" :
      rejected ? "Resultados locales pendientes de revisión: "+rejected : "Conectado · tiempos enviados provisionales";
  } catch (e) {
    if (e.status === 401 && session) session.expires=0;
    el("status").textContent="Sin sincronizar · "+e.message+". Tus tiempos siguen en el dispositivo.";
  } finally { busy=false; if(refreshQueued){refreshQueued=false; void update(true);} }
}
window.boardOpened = () => {active=true; update(true);};
el("close").onclick=()=>{active=false; bridge("close");};
el("filter").onclick=()=>{filtered=!filtered; el("filter").textContent=filtered ? "Vehículo actual" : "Todos los vehículos"; update(true);};
el("reload").onclick=()=>update(true);
el("save").onclick=async()=>{
  try { await authenticate(); await rpc("leaderboard_profile",{p_name:el("pilot").value.trim()}); await update(true); }
  catch(e){el("status").textContent="Nombre sin guardar · "+e.message;}
};
new IntersectionObserver(([e])=>{if(active && e.isIntersecting) page().catch(err=>el("status").textContent=err.message);}).observe(el("more"));
window.addEventListener("online",()=>update(true));
setInterval(()=>update(), 30000);
update();
