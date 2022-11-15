INSERT INTO users
VALUES
    ('00000000-0000-0000-0000-000000000001', 'user1', 'pass1'),
    ('00000000-0000-0000-0000-000000000002', 'user2', 'pass2'),
    ('00000000-0000-0000-0000-000000000003', 'user3', 'pass3'),
    ('00000000-0000-0000-0000-000000000004', 'user4', 'pass4'),
    ('00000000-0000-0000-0000-000000000005', 'user5', 'pass5')
ON CONFLICT DO NOTHING;

INSERT INTO tasks
VALUES
    ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', '2001-02-03', 'Task 1'),
    ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', '2001-02-03', 'Task 2'),
    ('00000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000001', '2001-02-03', 'Task 3'),
    ('00000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000001', '2001-02-03', 'Task 4'),
    ('00000000-0000-0000-0000-000000000005', '00000000-0000-0000-0000-000000000001', '2001-02-03', 'Task 5'),
    ('00000000-0000-0000-0000-000000000006', '00000000-0000-0000-0000-000000000001', '2001-02-03', 'Task 6'),
    ('00000000-0000-0000-0000-000000000007', '00000000-0000-0000-0000-000000000002', '2001-02-03', 'Task 7'),
    ('00000000-0000-0000-0000-000000000008', '00000000-0000-0000-0000-000000000002', '2001-02-03', 'Task 8'),
    ('00000000-0000-0000-0000-000000000009', '00000000-0000-0000-0000-000000000002', '2001-02-03', 'Task 9'),
    ('00000000-0000-0000-0000-000000000010', '00000000-0000-0000-0000-000000000003', '2001-02-03', 'Task 10'),
    ('00000000-0000-0000-0000-000000000011', '00000000-0000-0000-0000-000000000003', '2001-02-03', 'Task 11'),
    ('00000000-0000-0000-0000-000000000012', '00000000-0000-0000-0000-000000000003', '2001-02-03', 'Task 12')
ON CONFLICT DO NOTHING;

INSERT INTO set_title_events
VALUES
    ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', '2001-02-03', '00000000-0000-0000-0000-000000000001', 'Task 1 title 2'),
    ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', '2001-02-03', '00000000-0000-0000-0000-000000000001', 'Task 1 title 3'),
    ('00000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000001', '2001-02-03', '00000000-0000-0000-0000-000000000001', 'Task 1 title 4')
ON CONFLICT DO NOTHING;

INSERT INTO set_task_done_events
VALUES
    ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', '2001-02-04', '00000000-0000-0000-0000-000000000001', true),
    ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', '2001-02-04', '00000000-0000-0000-0000-000000000001', true),
    ('00000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000001', '2001-02-04', '00000000-0000-0000-0000-000000000001', true),
    ('00000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000001', '2001-02-04', '00000000-0000-0000-0000-000000000001', true),
    ('00000000-0000-0000-0000-000000000005', '00000000-0000-0000-0000-000000000001', '2001-02-06', '00000000-0000-0000-0000-000000000001', true),
    ('00000000-0000-0000-0000-000000000006', '00000000-0000-0000-0000-000000000001', '2001-02-08', '00000000-0000-0000-0000-000000000002', true),
    ('00000000-0000-0000-0000-000000000007', '00000000-0000-0000-0000-000000000001', '2001-02-10', '00000000-0000-0000-0000-000000000003', true),
    ('00000000-0000-0000-0000-000000000008', '00000000-0000-0000-0000-000000000001', '2001-02-05', '00000000-0000-0000-0000-000000000001', false),
    ('00000000-0000-0000-0000-000000000009', '00000000-0000-0000-0000-000000000001', '2001-02-07', '00000000-0000-0000-0000-000000000001', false),
    ('00000000-0000-0000-0000-000000000010', '00000000-0000-0000-0000-000000000001', '2001-02-09', '00000000-0000-0000-0000-000000000002', false),
    ('00000000-0000-0000-0000-000000000011', '00000000-0000-0000-0000-000000000001', '2001-02-09', '00000000-0000-0000-0000-000000000002', false),
    ('00000000-0000-0000-0000-000000000012', '00000000-0000-0000-0000-000000000001', '2001-02-09', '00000000-0000-0000-0000-000000000002', false),
    ('00000000-0000-0000-0000-000000000013', '00000000-0000-0000-0000-000000000001', '2001-02-09', '00000000-0000-0000-0000-000000000002', false)
ON CONFLICT DO NOTHING;

INSERT INTO set_task_archived_events
VALUES
    ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', '2001-02-04', '00000000-0000-0000-0000-000000000001', true),
    ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', '2001-02-04', '00000000-0000-0000-0000-000000000001', true),
    ('00000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000001', '2001-02-04', '00000000-0000-0000-0000-000000000001', true),
    ('00000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000001', '2001-02-04', '00000000-0000-0000-0000-000000000001', true),
    ('00000000-0000-0000-0000-000000000005', '00000000-0000-0000-0000-000000000001', '2001-02-06', '00000000-0000-0000-0000-000000000001', true),
    ('00000000-0000-0000-0000-000000000006', '00000000-0000-0000-0000-000000000001', '2001-02-08', '00000000-0000-0000-0000-000000000002', true),
    ('00000000-0000-0000-0000-000000000007', '00000000-0000-0000-0000-000000000001', '2001-02-10', '00000000-0000-0000-0000-000000000003', true),
    ('00000000-0000-0000-0000-000000000008', '00000000-0000-0000-0000-000000000001', '2001-02-05', '00000000-0000-0000-0000-000000000001', false),
    ('00000000-0000-0000-0000-000000000009', '00000000-0000-0000-0000-000000000001', '2001-02-07', '00000000-0000-0000-0000-000000000001', false),
    ('00000000-0000-0000-0000-000000000010', '00000000-0000-0000-0000-000000000001', '2001-02-09', '00000000-0000-0000-0000-000000000002', false),
    ('00000000-0000-0000-0000-000000000011', '00000000-0000-0000-0000-000000000001', '2001-02-09', '00000000-0000-0000-0000-000000000002', false),
    ('00000000-0000-0000-0000-000000000012', '00000000-0000-0000-0000-000000000001', '2001-02-09', '00000000-0000-0000-0000-000000000002', false),
    ('00000000-0000-0000-0000-000000000013', '00000000-0000-0000-0000-000000000001', '2001-02-09', '00000000-0000-0000-0000-000000000002', false)
ON CONFLICT DO NOTHING;

INSERT INTO schedule_events
VALUES
    ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', '2001-02-03', '00000000-0000-0000-0000-000000000001', '2001-03-04'),
    ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', '2001-03-04', '00000000-0000-0000-0000-000000000001', '2001-03-06'),
    ('00000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000001', '2001-03-02', '00000000-0000-0000-0000-000000000001', '2001-03-05')
ON CONFLICT DO NOTHING;

INSERT INTO add_dependency_events
VALUES
    ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', '2001-02-03', '00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000002'),
    ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', '2001-02-03', '00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000003'),
    ('00000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000001', '2001-02-03', '00000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000001')
ON CONFLICT DO NOTHING;

INSERT INTO remove_dependency_events
VALUES
    ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', '2001-02-04', '00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000002')
ON CONFLICT DO NOTHING;

INSERT INTO tags
VALUES
    ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', 'today', false),
    ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000002', 'today', false),
    ('00000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000003', 'today', false),
    ('00000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000004', 'today', false),
    ('00000000-0000-0000-0000-000000000005', '00000000-0000-0000-0000-000000000005', 'today', false),
    ('00000000-0000-0000-0000-000000000006', '00000000-0000-0000-0000-000000000001', 'tag1', false),
    ('00000000-0000-0000-0000-000000000007', '00000000-0000-0000-0000-000000000001', 'tag2', true),
    ('00000000-0000-0000-0000-000000000008', '00000000-0000-0000-0000-000000000001', 'tag3', false),
    ('00000000-0000-0000-0000-000000000009', '00000000-0000-0000-0000-000000000002', 'tag4', false)
ON CONFLICT DO NOTHING;

INSERT INTO add_tag_events
VALUES
    ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', '2001-02-03', '00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000004', 0, false),
    ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', '2001-02-03', '00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000006', 0, false),
    ('00000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000001', '2001-02-03', '00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000009', 0, false),
    ('00000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000001', '2001-02-03', '00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000007', 0, true),
    ('00000000-0000-0000-0000-000000000005', '00000000-0000-0000-0000-000000000001', '2001-02-04', '00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000008', 0, false),
    ('00000000-0000-0000-0000-000000000006', '00000000-0000-0000-0000-000000000002', '2001-02-03', '00000000-0000-0000-0000-000000000007', '00000000-0000-0000-0000-000000000009', 1, false),
    ('00000000-0000-0000-0000-000000000007', '00000000-0000-0000-0000-000000000002', '2001-02-03', '00000000-0000-0000-0000-000000000008', '00000000-0000-0000-0000-000000000009', 0, false)
ON CONFLICT DO NOTHING;

INSERT INTO remove_tag_events
VALUES
    ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', '2001-02-04', '00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000002'),
    ('00000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000001', '2001-02-03', '00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000003'),
    ('00000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000002', '2001-02-04', '00000000-0000-0000-0000-000000000007', '00000000-0000-0000-0000-000000000004')
ON CONFLICT DO NOTHING;

INSERT INTO perms
VALUES
    ('00000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000001', false, false, false, false),
    ('00000000-0000-0000-0000-000000000009', '00000000-0000-0000-0000-000000000001', false, false, false, false)
ON CONFLICT DO NOTHING;

INSERT INTO add_comment_events
VALUES
    ('00000000-0000-0000-0001-000000000001', '00000000-0000-0000-0000-000000000001', '2001-02-03', '00000000-0000-0000-0000-000000000001', 'Comment 1'),
    ('00000000-0000-0000-0001-000000000002', '00000000-0000-0000-0000-000000000001', '2001-02-04', '00000000-0000-0000-0000-000000000001', 'Comment 2 pre-edit'),
    ('00000000-0000-0000-0001-000000000003', '00000000-0000-0000-0000-000000000001', '2001-02-06', '00000000-0000-0000-0000-000000000001', 'Comment 4'),
    ('00000000-0000-0000-0001-000000000004', '00000000-0000-0000-0000-000000000001', '2001-02-05', '00000000-0000-0000-0000-000000000001', 'Comment 3')
ON CONFLICT DO NOTHING;

INSERT INTO edit_comment_events
VALUES
    ('00000000-0000-0000-0001-000000000005', '00000000-0000-0000-0000-000000000001', '2001-02-05', '00000000-0000-0000-0001-000000000002', 'Comment 2 mid-edit'),
    ('00000000-0000-0000-0001-000000000006', '00000000-0000-0000-0000-000000000001', '2001-02-07', '00000000-0000-0000-0001-000000000002', 'Comment 2 last-edit')
ON CONFLICT DO NOTHING;
