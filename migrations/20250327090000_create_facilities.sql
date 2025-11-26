create type facility_type as enum (
    'Artcc',
    'Tracon',
    'AtctTracon',
    'AtctRapcon',
    'Atct'
);

create table facilities
(
    id              text           not null,
    facility_type   facility_type  not null,
    parent_id       text,
    root_artcc_id   text           not null,
    name            text           not null,
    last_updated_at timestamptz    not null,
    first_seen      timestamptz    not null default now(),
    is_active       bool           not null default true,
    constraint facilities_pkey primary key (id),
    constraint facilities_parent_fk foreign key (parent_id) references facilities (id),
    constraint facilities_root_fk foreign key (root_artcc_id) references facilities (id),
    constraint facilities_root_self check (
        (facility_type = 'Artcc' and parent_id is null and root_artcc_id = id)
        or facility_type != 'Artcc'
    ),
    constraint facilities_parent_type check (
        (facility_type in ('Tracon','AtctTracon','AtctRapcon') and parent_id is not null)
        or (facility_type = 'Atct' and parent_id is not null)
        or (facility_type = 'Artcc' and parent_id is null)
    )
);

create index idx_facilities_root_active on facilities (root_artcc_id, is_active);
create index idx_facilities_parent on facilities (parent_id);
create index idx_facilities_type on facilities (facility_type);

create table facility_positions
(
    id          text        not null,
    facility_id text        not null,
    name        text        not null,
    callsign    text,
    radio_name  text,
    frequency   bigint,
    starred     bool        not null,
    last_updated_at timestamptz not null,
    first_seen  timestamptz not null default now(),
    is_active   bool        not null default true,
    constraint facility_positions_pkey primary key (id),
    constraint facility_positions_facility_fk foreign key (facility_id) references facilities (id)
);

create index idx_facility_positions_facility on facility_positions (facility_id);
create index idx_facility_positions_active on facility_positions (is_active);
