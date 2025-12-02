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

create table callsign_sessions
(
    id           uuid        not null,
    prefix       text        not null,
    suffix       text        not null,
    start_time   timestamptz not null,
    end_time     timestamptz,
    duration     interval,
    last_seen    timestamptz not null default now(),
    is_active    bool        not null,
    created_at   timestamptz not null default now(),
    constraint callsign_sessions_pkey primary key (id)
);

create unique index uq_callsign_sessions_active_prefix_suffix
    on callsign_sessions (prefix, suffix)
    where is_active = true;

create index idx_callsign_sessions_active on callsign_sessions (is_active);
create index idx_callsign_sessions_last_seen on callsign_sessions (last_seen);

create table position_sessions
(
    id           uuid        not null,
    position_id  text        not null,
    start_time   timestamptz not null,
    end_time     timestamptz,
    duration     interval,
    last_seen    timestamptz not null default now(),
    is_active    bool        not null,
    created_at   timestamptz not null default now(),
    constraint position_sessions_pkey primary key (id)
);

create unique index uq_position_sessions_active_position
    on position_sessions (position_id)
    where is_active = true;

create index idx_position_sessions_active on position_sessions (is_active);
create index idx_position_sessions_last_seen on position_sessions (last_seen);

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
    connected_callsign   text                 not null,
    primary_position_id  text                 not null,
    callsign_session_id  uuid                 not null,
    position_session_id  uuid                 not null,
    constraint controller_sessions_pkey primary key (id),
    constraint controller_sessions_callsign_session_fk foreign key (callsign_session_id) references callsign_sessions (id),
    constraint controller_sessions_position_session_fk foreign key (position_session_id) references position_sessions (id)
);

-- Only one active session per controller ID at a time.
create unique index uq_controller_sessions_active_cid
    on controller_sessions (cid)
    where is_active = true;

create index idx_controller_sessions_active on controller_sessions (is_active, cid);
create index idx_controller_sessions_end_time on controller_sessions (end_time) where is_active = false;
create index idx_controller_sessions_login_time on controller_sessions (login_time);
create index idx_controller_sessions_callsign_session_id on controller_sessions (callsign_session_id);
create index idx_controller_sessions_position_session_id on controller_sessions (position_session_id);

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

create table datafeed_payloads
(
    id                  uuid        not null,
    updated_at          timestamptz not null,
    payload_compressed  bytea       not null,
    original_size_bytes integer     not null,
    compression_algo    text        not null default 'zstd',
    created_at          timestamptz not null,
    constraint datafeed_payloads_pkey primary key (id),
    constraint uq_datafeed_payloads_updated unique (updated_at)
);

-- We already store this compressed; avoid an extra TOAST compression pass.
alter table datafeed_payloads
    alter column payload_compressed set storage external;

create index idx_datafeed_payloads_updated_at on datafeed_payloads (updated_at);

create table datafeed_messages
(
    id           uuid        not null,
    queue_id     uuid        not null,
    payload_id   uuid        not null,
    enqueued_at  timestamptz not null,
    processed_at timestamptz not null default now(),
    constraint datafeed_messages_pkey primary key (id),
    constraint datafeed_messages_payload_fk foreign key (payload_id) references datafeed_payloads (id)
);

create index idx_datafeed_messages_processed_at on datafeed_messages (processed_at);
