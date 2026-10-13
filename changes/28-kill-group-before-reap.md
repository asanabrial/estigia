section: The harness

- **A process group is killed before its leader is reaped.** Issue #28. The reviewer runner, the bounded process
  runner and the transport's deadline path used to reap an exited child and signal its process group afterwards,
  when the kernel could already have reused the group ID. Each now observes the exit without reaping
  (`waitid` with `WNOWAIT` on Unix), signals the group, then reaps. A test-build assertion in the group kill
  fails any test that signals after the reap. Behaviour on a running child is unchanged.
