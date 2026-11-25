"""Exercise the actual Auth, PostgREST and Edge Runtime of a disposable local stack."""
import json
import os
from urllib.error import HTTPError
from urllib.request import Request, urlopen

base = os.environ["API_URL"]
assert base.startswith(("http://127.0.0.1:", "http://localhost:"))
key = os.environ["ANON_KEY"]


def call(path, payload, jwt=None):
    headers = {"Content-Type": "application/json", "apikey": key}
    if jwt:
        headers["Authorization"] = "Bearer " + jwt
    req = Request(base + path, json.dumps(payload).encode(), headers)
    try:
        with urlopen(req, timeout=30) as response:
            return response.status, json.load(response)
    except HTTPError as error:
        return error.code, None


def guest():
    code, session = call("/auth/v1/signup", {})
    assert code == 200 and session["user"]["is_anonymous"]
    code, renewed = call("/auth/v1/token?grant_type=refresh_token", {"refresh_token": session["refresh_token"]})
    assert code == 200 and renewed["user"]["id"] == session["user"]["id"]
    return renewed["access_token"]


ipad, android = guest(), guest()
submit = "/functions/v1/submit-stage-time"
race = dict(entry_id="lb1-12345678901234567890", stage_id="chirdonhead", vehicle_id="toyota-corolla-wrc",
            driving_ms=120000, penalty_ms=3000, total_ms=123000, sim_steps=86400,
            splits_ms=[40000, 80000], finished=True, finish_crossed_forward=True)
assert call(submit, race)[0] == 401
assert call(submit, race, "invalid")[0] == 401
assert call(submit, dict(race, player_id="spoofed"), ipad)[0] == 422
assert call(submit, dict(race, verified=True), ipad)[0] == 422
assert call(submit, dict(race, splits_ms=[]), ipad)[0] == 422
assert call(submit, dict(race, vehicle_id="unknown"), ipad)[0] == 422
assert call("/rest/v1/rpc/leaderboard_profile", {"p_name": "Piloto iPad"}, ipad)[0] == 200
code, result = call(submit, race, ipad)
assert code == 201 and result["verified"] is False, code
code, retry = call(submit, race, ipad)
assert code == 200 and retry["id"] == result["id"]
assert call("/rest/v1/stage_times", {"stage_id": "chirdonhead"}, ipad)[0] in (401, 403)
query = {"p_stage": "chirdonhead", "p_vehicle": "toyota-corolla-wrc", "p_offset": 0}
code, ranking = call("/rest/v1/rpc/leaderboard_page", query, android)
assert code == 200 and ranking["mine"] is None and ranking["count"] == 1
record = ranking["rows"][0]
assert (record["display_name"], record["position"], record["total_ms"], record["verified"]) == ("Piloto iPad", 1, 123000, False)
assert call("/rest/v1/rpc/leaderboard_page", query, ipad)[1]["mine"]["position"] == 1
assert call("/rest/v1/rpc/leaderboard_page", dict(query, p_offset=25), android)[1]["rows"] == []
print("PASS: two identities, refresh, JWT validation, rejected writes, idempotency, cross-session provisional ranking")
