CREATE VIEW v_tasks_archived AS
SELECT
    t.id as task_id,
    (
        (
            SELECT COALESCE(MAX(ate.date), 0)
            FROM archive_task_events ate
            WHERE ate.task_id = t.id
        ) > (
            SELECT COALESCE(MAX(ute.date), 0)
            FROM unarchive_task_events ute
            WHERE ute.task_id = t.id
        )
    ) as archived
FROM
    tasks t;

CREATE VIEW v_tasks_tags AS
SELECT DISTINCT
    t.id as task_id,
    ate.tag_id as tag_id
FROM
    tasks t
INNER JOIN add_tag_events ate
    ON ate.task_id = t.id
WHERE NOT EXISTS (
    SELECT NULL
    FROM remove_tag_events rte
    WHERE rte.add_tag_id = ate.id
);
