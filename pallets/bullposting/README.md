License: MIT-0

## Release

Polkadot SDK stable2409

# Bullposting
Through the Bullposting pallet's extrinsics, a user can submit a post (in the form of a string), along with bonding some tokens.
Other users of the chain can vote on this submission (during the voting period) by freezing some tokens.

Once the voting period ends, the result will be determined to be Bullish, Bearish, or a tie. 
Ties result in no change, and the effects of Bullish or Bearish are configurable in the runtime 
(eg. reward the submitter with +50% of their bond, or slash 100% of their bond). Voters receive no reward for voting.

Once the voting period has ended, anyone can end the post with `end_post()`, calculating the final verdict and rewarding/penalizing 
the submitter accordingly. Following this, anyone can resolve the post, unfreezing the votes of voters. The maximum 
number of votes that can be unfrozen per attempt is defined in the runtime. Users may need to call `resolve_post()` 
multiple times to fully unfreeze all votes on a post.