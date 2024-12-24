  * Add more documentation (both README and code docs), examples, code clean up.

  * The ahtml namespace is full of all-caps variables like `P_META`, split
    those vs. the rest into separate namespaces.

  * proc macro idea to add auto-propagation for Result::Err values?

  * Add tests for StillVec (for testing under Miri/ASAN). (Some real
    world code has already been tested via Miri without issues.)
