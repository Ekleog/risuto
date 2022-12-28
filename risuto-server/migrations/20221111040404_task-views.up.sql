CREATE VIEW v_tasks_archived AS
SELECT DISTINCT ON (e.task_id)
    e.task_id AS task_id,
    e.d_bool AS archived
FROM events e
WHERE e.d_type = 'set_archived'
ORDER BY e.task_id, e.date DESC;

CREATE VIEW v_tasks_comments AS
SELECT DISTINCT ON (task_id, comment_id)
    e.task_id AS task_id,
    (CASE e.d_type
        WHEN 'add_comment' THEN e.id
        ELSE e.d_parent_id
    END) AS comment_id,
    e.d_text AS text
FROM events e
WHERE (e.d_type = 'add_comment' OR e.d_type = 'edit_comment')
ORDER BY task_id, comment_id, e.date DESC;

CREATE VIEW v_tasks_text AS
SELECT
    task_id,
    string_agg(text, '\n') AS text
FROM v_tasks_comments
GROUP BY task_id;

CREATE VIEW v_tasks_tags AS
SELECT DISTINCT ON (task_id, tag_id)
    e.task_id AS task_id,
    e.d_tag_id AS tag_id,
    (e.d_type = 'add_tag') AS is_in,
    e.d_bool AS backlog
FROM events e
WHERE (e.d_type = 'add_tag' OR e.d_type = 'remove_tag')
ORDER BY task_id, tag_id, e.date DESC;

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
