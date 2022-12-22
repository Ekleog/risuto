CREATE VIEW v_tasks_archived AS
SELECT
    t.id as task_id,
    COALESCE(
        (
            SELECT e.new_val_bool
            FROM events e
            WHERE e.task_id = t.id AND e.type = 'set_archived'
            ORDER BY e.date DESC
            LIMIT 1
        ),
        false
    ) as archived
FROM
    tasks t;

CREATE VIEW v_tasks_tags AS
SELECT DISTINCT
    t.id as task_id,
    adds.tag_id as tag_id
FROM
    tasks t
INNER JOIN events adds
    ON adds.task_id = t.id AND adds.type = 'add_tag'
WHERE NOT EXISTS (
    SELECT NULL
    FROM events rms
    WHERE rms.tag_id = adds.tag_id
    AND rms.task_id = adds.task_id
    AND rms.type = 'remove_tag'
    AND rms.date > adds.date
);

CREATE VIEW v_tags_users AS
SELECT
    tag_id,
    user_id,
    BOOL_OR(can_edit) AS can_edit,
    BOOL_OR(can_triage) AS can_triage,
    BOOL_OR(can_relabel_to_any) AS can_relabel_to_any,
    BOOL_OR(can_comment) AS can_comment
FROM (
    (
        SELECT
            id as tag_id,
            owner_id as user_id,
            true as can_edit,
            true as can_triage,
            true as can_relabel_to_any,
            true as can_comment
        FROM
            tags
    )
    UNION
    (
        SELECT
            t.id as tag_id,
            p.user_id as user_id,
            p.can_edit,
            p.can_triage,
            p.can_relabel_to_any,
            p.can_comment
        FROM
            tags t
        INNER JOIN perms p
            ON p.tag_id = t.id
    )
) AS the_table_because_postgres_needs_this_name
GROUP BY (tag_id, user_id);

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
            vtu.user_id as user_id,
            vtu.can_edit,
            vtu.can_triage,
            vtu.can_relabel_to_any,
            vtu.can_comment
        FROM
            tasks t
        INNER JOIN v_tasks_tags vtt
            ON vtt.task_id = t.id
        INNER JOIN v_tags_users vtu
            ON vtu.tag_id = vtt.tag_id
    )
) AS the_table_because_postgres_needs_this_name
GROUP BY (task_id, user_id);
