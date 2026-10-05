# Tasks in started work

Tasks linked to `active`, `ready`, or `processing` work are automatically moved
from `backlog` to `ready`. Starting or resuming paused work also queues its
existing backlog tasks. Work activation queues tasks in the same transaction as
the status change; new single and bulk links queue only the newly linked tasks.

This is a one-time scheduling event, not a continuously enforced state. You can
move a task back to `backlog` to hold it. Editing work details, repeating its
current status, or submitting duplicate links does not queue it again. Explicitly
activating or pausing and resuming the work does queue its backlog again.

Running, review, completed, and cancelled tasks are not changed. Tasks without
work links remain in backlog when created. Dependencies still prevent queued
tasks from being offered to or claimed by workers until their blockers are done.
