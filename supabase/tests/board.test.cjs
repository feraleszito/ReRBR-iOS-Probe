const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');

test('offline finish, lost acknowledgement, retry, secure refresh and remote ranking', async () => {
  const run = {entry_id:'lb1-00000000000000000001',stage_id:'chirdonhead',vehicle_id:'toyota-corolla-wrc'};
  const server = new Map();
  let online=false, loseAck=true, registrations=0, refreshes=0, queue=[run], stored='';
  const elements = new Map();
  function element(id) {
    if(!elements.has(id)) elements.set(id,{textContent:'',rows:[],replaceChildren(){this.rows=[];},appendChild(row){this.rows.push(row);}});
    return elements.get(id);
  }
  const window={addEventListener(){},RbrBoard:{call(op,value){
    if(op==='selection') return JSON.stringify(run);
    if(op==='pending') return JSON.stringify(queue);
    if(op==='session') return stored;
    if(op==='persist'){stored=value;return 'ok';}
    if(op==='ack'){queue=queue.filter(r=>r.entry_id!==value);return 'ok';}
    throw Error(op);
  }}};
  async function fetch(url,req) {
    if(!online) throw Error('offline');
    const body=JSON.parse(req.body);
    const json = data=>({ok:true,json:async()=>data});
    if(url.includes('/auth/v1/')){
      if(url.endsWith('/signup')) registrations++; else refreshes++;
      return json({access_token:'access',refresh_token:'refresh',expires_in:3600});
    }
    assert.equal(req.headers.Authorization,'Bearer access');
    if(url.endsWith('leaderboard_profile')) return json('Piloto iPad');
    if(url.endsWith('submit-stage-time')){
      server.set(body.entry_id,{position:1,display_name:'Piloto iPad',total_ms:123000,vehicle_id:body.vehicle_id,verified:false});
      if(loseAck){loseAck=false;throw Error('response lost');}
      return json({id:'1',verified:false});
    }
    assert.ok(url.endsWith('leaderboard_page'));
    return json({rows:[...server.values()],mine:null,count:server.size});
  }
  const ctx=vm.createContext({window,document:{getElementById:element,createElement:()=>({})},fetch,
    AbortSignal,IntersectionObserver:class{observe(){}},setInterval(){}});
  for(const f of ['config.js','board.js']) vm.runInContext(fs.readFileSync('mobile/leaderboard/'+f,'utf8'),ctx);
  await new Promise(resolve=>setImmediate(resolve));
  assert.equal(queue.length,1);
  online=true;
  await vm.runInContext('active=true; update(true)',ctx);
  assert.equal(queue.length,1);
  assert.equal(server.size,1);
  await vm.runInContext('session.expires=0; update(true)',ctx);
  assert.equal(queue.length,0);
  assert.equal(server.size,1);
  assert.equal(registrations,1);
  assert.equal(refreshes,1);
  assert.match(element('rows').rows[0].textContent,/Piloto iPad.*Provisional/);
});
