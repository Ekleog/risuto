CREATE INDEX events_task_ordered_by_type
ON events (d_type, date DESC, task_id);
