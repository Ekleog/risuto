CREATE VIEW v_tasks_archived AS
SELECT
    t.id as task_id,
    COALESCE(
        (
            SELECT stae.now_archived
            FROM set_task_archived_events stae
            WHERE stae.task_id = t.id
            ORDER BY stae.date DESC
            LIMIT 1
        ),
        false
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
    AND rte.task_id = ate.task_id
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
        tag.owner_id as user_id
    FROM
        tasks t
    INNER JOIN v_tasks_tags vtt
        ON vtt.task_id = t.id
    INNER JOIN tags tag
        ON tag.id = vtt.tag_id
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
