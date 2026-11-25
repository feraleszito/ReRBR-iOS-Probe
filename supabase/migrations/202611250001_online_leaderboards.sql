begin;

-- Existing records retain their original values and have no client submission ID.
alter table public.stage_times add column submission_id text;
create unique index stage_times_submission_uidx on public.stage_times(player_id, submission_id);
alter table public.players enable row level security;
alter table public.stage_times enable row level security;
revoke insert, update, delete, truncate on public.players, public.stage_times from public, anon, authenticated;

create schema if not exists leaderboard_private;
revoke all on schema leaderboard_private from public, anon, authenticated;
create table leaderboard_private.stages (
    id text primary key, name text not null, splits integer not null check (splits between 0 and 128)
);
create table leaderboard_private.vehicles (id text primary key, name text not null);
create table leaderboard_private.limits (
    player_id uuid primary key references auth.users(id) on delete cascade,
    started timestamptz not null, requests integer not null
);

-- The app's Chirdonhead record fixture contains two intermediate splits.
insert into leaderboard_private.stages values ('chirdonhead','Chirdonhead',2);
insert into leaderboard_private.vehicles values
    ('toyota-corolla-wrc','Toyota Corolla WRC'), ('ford-focus-rs-wrc-2006','Ford Focus RS WRC 2006');

create function public.leaderboard_profile(p_name text default null)
returns text language plpgsql security definer set search_path = '' as $$
declare uid uuid := auth.uid(); result text;
begin
    if uid is null then raise insufficient_privilege; end if;
    if p_name is not null and (length(trim(p_name)) not between 3 and 24 or p_name ~ '[[:cntrl:]]') then
        raise exception 'invalid display name' using errcode = '22023';
    end if;
    insert into public.players(id, display_name) values (uid, coalesce(trim(p_name),'Piloto ' || left(uid::text,8)))
    on conflict(id) do nothing;
    if p_name is not null then update public.players set display_name=trim(p_name) where id=uid; end if;
    select display_name into strict result from public.players where id=uid;
    return result;
end $$;
revoke all on function public.leaderboard_profile(text) from public, anon;
grant execute on function public.leaderboard_profile(text) to authenticated;

-- Only the Edge Function can invoke this RPC. A request budget, idempotency
-- check and insert share one transaction and one per-player row lock.
create function public.leaderboard_submit(p_uid uuid, p_submission text, p_stage text, p_vehicle text,
    p_time integer, p_penalty integer, p_splits integer[])
returns jsonb language plpgsql security definer set search_path = '' as $$
declare budget integer; stage leaderboard_private.stages; vehicle leaderboard_private.vehicles;
    previous public.stage_times; result bigint;
begin
    insert into leaderboard_private.limits as l values (p_uid, now(), 1)
    on conflict(player_id) do update set
      started = case when l.started <= now()-interval '1 minute' then now() else l.started end,
      requests = case when l.started <= now()-interval '1 minute' then 1 else least(l.requests+1,13) end
    returning requests into budget;
    if budget > 12 then return jsonb_build_object('status',429); end if;
    select * into previous from public.stage_times where player_id=p_uid and submission_id=p_submission;
    if found then
        if (previous.stage_id,previous.vehicle_id,previous.time_ms,previous.penalty_ms)
            is distinct from (p_stage,p_vehicle,p_time,p_penalty) then return jsonb_build_object('status',409); end if;
        return jsonb_build_object('status',200,'id',previous.id::text,'verified',previous.verified);
    end if;
    select * into stage from leaderboard_private.stages where id=p_stage;
    select * into vehicle from leaderboard_private.vehicles where id=p_vehicle;
    if stage.id is null or vehicle.id is null or p_splits is null or cardinality(p_splits) <> stage.splits
       or p_time is null or p_time not between 1 and 86400000 or p_penalty is null or p_penalty not between 0 and 86400000
       or p_submission is null or p_submission !~ '^lb1-[0-9a-f]{20}$'
       or exists (select 1 from unnest(p_splits) with ordinality as s(ms,i)
                  where ms is null or ms <= 0 or ms >= p_time or ms <= p_splits[(i-1)::int]) then
        return jsonb_build_object('status',422);
    end if;
    insert into public.players(id,display_name) values (p_uid,'Piloto ' || left(p_uid::text,8)) on conflict(id) do nothing;
    insert into public.stage_times(player_id, submission_id, stage_id, stage_name, vehicle_id, vehicle_name,
                                   time_ms, penalty_ms, verified)
    values (p_uid,p_submission,stage.id,stage.name,vehicle.id,vehicle.name,p_time,p_penalty,false) returning id into result;
    return jsonb_build_object('status',201,'id',result::text,'verified',false);
end $$;
revoke all on function public.leaderboard_submit(uuid,text,text,text,integer,integer,integer[]) from public,anon,authenticated;
grant execute on function public.leaderboard_submit(uuid,text,text,text,integer,integer,integer[]) to service_role;

create function public.leaderboard_page(p_stage text,p_vehicle text default null,p_offset integer default 0)
returns jsonb language sql stable security invoker set search_path = '' as $$
    with best as (
        select distinct on (t.player_id) t.id,t.player_id,p.display_name,t.vehicle_id,t.verified,
               t.time_ms::bigint+t.penalty_ms as total_ms
        from public.stage_times t join public.players p on p.id=t.player_id
        where t.stage_id=p_stage and (p_vehicle is null or t.vehicle_id=p_vehicle)
          and t.time_ms>0 and t.penalty_ms>=0
        order by t.player_id,total_ms,t.id
    ), ranked as (
        select *,rank() over(order by total_ms) as position from best
    ), page as (
        select * from ranked order by total_ms,id limit 25 offset greatest(p_offset,0)
    )
    select jsonb_build_object('rows',coalesce((select jsonb_agg(to_jsonb(page) order by total_ms,id) from page),'[]'::jsonb),
        'mine',(select to_jsonb(r) from ranked r where player_id=auth.uid()),'count',(select count(*) from ranked))
$$;
revoke all on function public.leaderboard_page(text,text,integer) from public,anon;
grant execute on function public.leaderboard_page(text,text,integer) to authenticated;
commit;
