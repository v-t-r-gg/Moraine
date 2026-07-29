//! Capture intake, durable spool processing & rebuildable discovery.
//!
//! Project run bundles remain canonical in `moraine-core`. Concrete listeners
//! are selected by the executable; this library owns event processing, not
//! operating-system registration or desktop state.

pub mod capture;
pub mod service;

pub use service::{
    count_run_records, event_already_seen, find_project_root_in_index, index_revision,
    list_project_runs, mark_event_seen, process_spool_file, read_index_projects, rebuild_index,
    rebuild_index_for_roots, rebuild_index_from_registry, rebuild_registered_index, spool_counts,
    write_spooled_payload, Event, MechanicalEvent, MAX_EVENT_BYTES, MAX_PENDING_EVENTS,
};
