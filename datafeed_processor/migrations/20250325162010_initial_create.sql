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

create table if not exists controller_sessions
(
    id                   uuid                 not null,
    login_time           timestamptz          not null,
    start_time           timestamptz          not null,
    end_time             timestamptz,
    duration             interval             not null,
    is_active            bool                 not null,
    is_observer          bool                 not null,
    cid                  integer              not null,
    name                 text                 not null,
    user_rating          user_rating          not null,
    requested_rating     user_rating          not null,
    callsign             text                 not null,
    vatsim_facility_type vatsim_facility_type not null,
    primary_frequency    integer              not null
);