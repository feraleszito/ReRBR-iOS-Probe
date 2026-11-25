begin;
insert into auth.users values ('00000000-0000-4000-a000-000000000001'),('00000000-0000-4000-a000-000000000002');
set local request.jwt.claim.sub='00000000-0000-4000-a000-000000000001';
set local role authenticated;
select public.leaderboard_profile('Piloto iPad');
do $$begin
    assert not has_table_privilege(current_user,'public.stage_times','insert');
    assert not has_function_privilege(current_user,'public.leaderboard_submit(uuid,text,text,text,integer,integer,integer[])','execute');
end $$;
set local role service_role;
do $$
declare u uuid := '00000000-0000-4000-a000-000000000001'; r jsonb; first_id text;
begin
    r := public.leaderboard_submit(u,'lb1-00000000000000000001','chirdonhead','toyota-corolla-wrc',120000,3000,array[40000,80000]);
    assert r->>'status'='201' and r->>'verified'='false';
    first_id := r->>'id';
    r := public.leaderboard_submit(u,'lb1-00000000000000000001','chirdonhead','toyota-corolla-wrc',120000,3000,array[40000,80000]);
    assert r->>'status'='200' and r->>'id'=first_id;
    assert public.leaderboard_submit(u,'lb1-00000000000000000002','chirdonhead','toyota-corolla-wrc',120000,0,array[40000])->>'status'='422';
    for i in 1..10 loop
        r := public.leaderboard_submit(u,'lb1-00000000000000000001','chirdonhead','toyota-corolla-wrc',120000,3000,array[40000,80000]);
    end loop;
    assert r->>'status'='429';
end $$;
set local request.jwt.claim.sub='00000000-0000-4000-a000-000000000002';
set local role authenticated;
do $$declare r jsonb; begin
    r := public.leaderboard_page('chirdonhead','toyota-corolla-wrc',0);
    assert r->>'count'='1' and r->'rows'->0->>'display_name'='Piloto iPad';
    assert r->'rows'->0->>'total_ms'='123000' and r->'rows'->0->>'position'='1';
    assert r->'mine'='null'::jsonb;
    assert public.leaderboard_page('chirdonhead','another-car',0)->>'count'='0';
end $$;
rollback;
