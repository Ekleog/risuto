CREATE VIEW v_tasks_archived AS
SELECT DISTINCT ON (e.task_id)
    e.task_id AS task_id,
    e.d_bool AS archived -- true on archived, false or non-existent on !archived
FROM events e
WHERE e.d_type = 'set_archived'
ORDER BY e.task_id, e.date DESC;

CREATE VIEW v_tasks_done AS
SELECT DISTINCT ON (e.task_id)
    e.task_id AS task_id,
    e.d_bool AS done -- true on done, false or non-existent on !done
FROM events e
WHERE e.d_type = 'set_done'
ORDER BY e.task_id, e.date DESC;

CREATE VIEW v_tasks_scheduled AS
SELECT DISTINCT ON (task_id, owner_id)
    task_id,
    owner_id,
    d_time AS time
FROM events
WHERE d_type = 'schedule_for'
ORDER BY task_id, owner_id, date DESC;

CREATE VIEW v_tasks_blocked AS
SELECT DISTINCT ON (task_id)
    task_id,
    d_time AS time
FROM events
WHERE d_type = 'blocked_until'
ORDER BY task_id, date DESC;

CREATE VIEW v_tasks_title AS
SELECT DISTINCT ON (t.id)
    t.id AS task_id,
    COALESCE(e.d_text, t.initial_title) AS title
FROM tasks t
LEFT JOIN events e
    ON t.id = e.task_id AND e.d_type = 'set_title'
ORDER BY t.id, e.date DESC;

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
    vtc.task_id,
    (
        setweight(to_tsvector(vtt.title), 'A') ||
        setweight(to_tsvector(string_agg(vtc.text, '\n')), 'D')
    ) AS text
FROM v_tasks_comments vtc
FULL JOIN v_tasks_title vtt
    ON vtc.task_id = vtt.task_id
GROUP BY vtc.task_id, vtt.title;

CREATE VIEW v_tasks_tags AS
SELECT DISTINCT ON (task_id, tag_id)
    e.task_id AS task_id,
    e.d_tag_id AS tag_id,
    (e.d_type = 'add_tag') AS is_in,
    e.d_bool AS backlog
FROM events e
WHERE (e.d_type = 'add_tag' OR e.d_type = 'remove_tag')
ORDER BY task_id, tag_id, e.date DESC;

CREATE VIEW v_tasks_is_tagged AS
SELECT
    task_id,
    BOOL_OR(COALESCE(is_in, false)) AS has_tag -- true on has_tag, false or non-existent on !has_tag
FROM v_tasks_tags vtt
GROUP BY task_id;

CREATE VIEW v_tags_users AS
SELECT
    tag_id,
    user_id,
    BOOL_OR(can_edit) AS can_edit,
    BOOL_OR(can_triage) AS can_triage,
    BOOL_OR(can_relabel_to_any) AS can_relabel_to_any,
    BOOL_OR(can_comment) AS can_comment,
    BOOL_OR(can_archive) AS can_archive
FROM (
    (
        SELECT
            id as tag_id,
            owner_id as user_id,
            true as can_edit,
            true as can_triage,
            true as can_relabel_to_any,
            true as can_comment,
            true as can_archive
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
            p.can_comment,
            p.can_archive
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
    BOOL_OR(can_comment) AS can_comment,
    BOOL_OR(can_archive) AS can_archive
FROM (
    (
        SELECT
            id as task_id,
            owner_id as user_id,
            true as can_edit,
            true as can_triage,
            true as can_relabel_to_any,
            true as can_comment,
            true as can_archive
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
            vtu.can_comment,
            vtu.can_archive
        FROM
            tasks t
        INNER JOIN v_tasks_tags vtt
            ON vtt.task_id = t.id
        INNER JOIN v_tags_users vtu
            ON vtu.tag_id = vtt.tag_id
    )
) AS the_table_because_postgres_needs_this_name
GROUP BY (task_id, user_id);
