New-item review admission
=========================

XIMG-046 admits a review card only when the acquisition lifecycle is
``Committed`` and carries verified ObjectStore evidence. The queue retains the
canonical media identity, source/account grouping, discovery time, verified
object reference, and ``New`` state. Interrupted, failed, policy-blocked, or
unverified work is rejected and never appears as a broken new card. Replays
retain the first queue record.
