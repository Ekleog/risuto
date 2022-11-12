CREATE VIEW v_tasks_archived AS
SELECT
    t.id as task_id,
    (
        (
            SELECT COALESCE(MAX(ate.date), '0001-01-01')
            FROM archive_task_events ate
            WHERE ate.task_id = t.id
        ) > (
            SELECT COALESCE(MAX(ute.date), '0001-01-01')
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
    WHERE rte.tag_id = ate.tag_id
    AND rte.date > ate.date
);

CREATE VIEW v_tasks_users AS
(
    SELECT
        id as task_id,
        owner_id as user_id
    FROM
        tasks
)
UNION
(
    SELECT
        t.id as task_id,
        p.user_id as user_id
    FROM
        tasks t
    INNER JOIN v_tasks_tags vtt
        ON vtt.task_id = t.id
    INNER JOIN perms p
        ON p.tag_id = vtt.tag_id
);
