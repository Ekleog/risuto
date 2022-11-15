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
SELECT
    task_id,
    user_id,
    BOOL_OR(can_edit) AS can_edit,
    BOOL_OR(can_triage) AS can_triage,
    BOOL_OR(can_relabel_to_any) AS can_relabel_to_any,
    BOOL_OR(can_comment) AS can_comment
FROM (
    (
        SELECT
            id as task_id,
            owner_id as user_id,
            true as can_edit,
            true as can_triage,
            true as can_relabel_to_any,
            true as can_comment
        FROM
            tasks
    )
    UNION
    (
        SELECT
            t.id as task_id,
            tag.owner_id as user_id,
            true as can_edit,
            true as can_triage,
            true as can_relabel_to_any,
            true as can_comment
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
            p.user_id as user_id,
            p.can_edit,
            p.can_triage,
            p.can_relabel_to_any,
            p.can_comment
        FROM
            tasks t
        INNER JOIN v_tasks_tags vtt
            ON vtt.task_id = t.id
        INNER JOIN perms p
            ON p.tag_id = vtt.tag_id
    )
) AS the_table_because_postgres_needs_this_name
GROUP BY (task_id, user_id);
