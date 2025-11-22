create type user_rating as enum (
    'observer',
    'student1',
    'student2',
    'student3',
    'controller1',
    'controller2',
    'controller3',
    'instructor1',
    'instructor2',
    'instructor3',
    'supervisor',
    'administrator'
    );

create type vatsim_facility_type as enum (
    'observer',
    'flight_service_station',
    'clearance_delivery',
    'ground',
    'tower',
    'approach_departure',
    'center'
    );

create table controller_sessions
(
    id                   uuid                 not null,
    login_time           timestamptz          not null,
    start_time           timestamptz          not null,
    end_time             timestamptz,
    duration             interval,
    last_seen            timestamptz          not null default now(),
    is_active            bool                 not null,
    is_observer          bool                 not null,
    cid                  integer              not null,
    name                 text                 not null,
    user_rating          user_rating          not null,
    requested_rating     user_rating          not null,
    callsign             text                 not null,
    vatsim_facility_type vatsim_facility_type not null,
    primary_frequency    integer              not null,
    constraint controller_sessions_pkey primary key (id)
);

-- Only one active session per controller ID at a time.
create unique index uq_controller_sessions_active_cid
    on controller_sessions (cid)
    where is_active = true;

create index idx_controller_sessions_active on controller_sessions (is_active, cid);
create index idx_controller_sessions_end_time on controller_sessions (end_time) where is_active = false;
create index idx_controller_sessions_login_time on controller_sessions (login_time);

create table datafeed_queue
(
    id          uuid        not null,
    updated_at  timestamptz not null,
    payload     jsonb       not null,
    created_at  timestamptz not null default now(),
    constraint datafeed_queue_pkey primary key (id)
);

create index idx_datafeed_queue_created_at on datafeed_queue (created_at);
create index idx_datafeed_queue_updated_at on datafeed_queue (updated_at);

create table datafeed_archive
(
    id                  uuid        not null,
    updated_at          timestamptz not null,
    payload_compressed  bytea       not null,
    original_size_bytes integer     not null,
    compression_algo    text        not null default 'zstd',
    created_at          timestamptz not null,
    processed_at        timestamptz not null default now(),
    constraint datafeed_archive_pkey primary key (id)
);

create index idx_datafeed_archive_processed_at on datafeed_archive (processed_at);
