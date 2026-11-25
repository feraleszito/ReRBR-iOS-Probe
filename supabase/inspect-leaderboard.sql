-- Run in the project's SQL Editor. Only catalog metadata is returned.
select c.oid::regclass::text as relation,
       c.relrowsecurity as rls_enabled,
       (select jsonb_agg(jsonb_build_object('name', a.attname,
           'type', format_type(a.atttypid, a.atttypmod), 'required', a.attnotnull,
           'identity', a.attidentity, 'default', pg_get_expr(d.adbin, d.adrelid)) order by a.attnum)
        from pg_attribute a left join pg_attrdef d on d.adrelid = a.attrelid and d.adnum = a.attnum
        where a.attrelid = c.oid and a.attnum > 0 and not a.attisdropped) as columns,
       (select jsonb_agg(pg_get_constraintdef(k.oid)) from pg_constraint k where k.conrelid = c.oid) as constraints,
       (select jsonb_agg(pg_get_indexdef(i.indexrelid)) from pg_index i where i.indrelid = c.oid) as indexes,
       (select jsonb_agg(to_jsonb(p)) from pg_policies p
        where p.schemaname = 'public' and p.tablename = c.relname) as policies,
       (select jsonb_agg(to_jsonb(g)) from information_schema.role_table_grants g
        where g.table_schema = 'public' and g.table_name = c.relname) as grants,
       (select jsonb_agg(pg_get_triggerdef(t.oid)) from pg_trigger t
        where t.tgrelid = c.oid and not t.tgisinternal) as triggers
from pg_class c
where c.relnamespace = 'public'::regnamespace and c.relname in ('players', 'stage_times')
order by c.relname;
