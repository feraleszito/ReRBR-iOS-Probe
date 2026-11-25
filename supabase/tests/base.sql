-- Disposable PostgreSQL CI fixture for the supplied pre-migration contract.
create role anon;
create role authenticated;
create role service_role bypassrls;
create schema auth;
create table auth.users(id uuid primary key);
create function auth.uid() returns uuid language sql stable as
$$select nullif(current_setting('request.jwt.claim.sub',true),'')::uuid$$;
grant usage on schema auth to authenticated;
create table public.players(id uuid primary key references auth.users on delete cascade,
 display_name text not null check(length(display_name) between 3 and 24),created_at timestamptz default now());
create table public.stage_times(id bigint generated always as identity primary key,
 player_id uuid references public.players on delete cascade,stage_id text not null,stage_name text not null,
 vehicle_id text not null,vehicle_name text not null,time_ms int check(time_ms>0),
 penalty_ms int default 0 check(penalty_ms>=0),created_at timestamptz default now(),verified boolean default false);
alter table public.players enable row level security;
alter table public.stage_times enable row level security;
grant select on public.players, public.stage_times to authenticated;
create policy read_players on public.players for select to authenticated using(true);
create policy read_times on public.stage_times for select to authenticated using(true);
