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

# Runtime Configuration
There are a number of constants that will need to be defined in the runtime, allowing you to configure how the pallet is used and how it will impact users.

## Rewards
- RewardStyle: A bool, this determines which reward mechanism is used if a post is determined to be Bullish. True uses RewardCoefficient, and False uses FlatReward.
- RewardCoefficient: A u32 determining the submitter's reward based on the size of their bond. A value of 100 is a 1x reward. 200 will give a 2x reward (eg. you bond 500 tokens, you will receive a 1000 token reward and end with 1500 tokens). Values between 0 and 100 can be used as well.
- FlatReward: A u32 determing the submitter's token reward, independent of their bond. Due to this, submitters will likely only bond the BondMinimum.

## Slashes
- SlashStyle: A bool, this determines which slashing mechanism is used if a post is determined to be Bearish. True uses SlashCoefficient, and False uses FlatSlash.
- SlashCoefficient: A u32 determining how much of the submitter's bond is slashed. A value of 100 slashes their entire bond. 50 will slash half of their bond (eg. you bond 500 tokens, 250 will be slashed and you will end with 250 tokens left). Only values between 0 and 100 can be used (anything over 100 will be treated as 100).
- FlatSlash: A u32 determing how many of the submitter's tokens are slashed, independent of their bond. If this is set higher than the bond of a post, only the submitter's full bond will be slashed (eg. if you bond 50 tokens and FlatSlash == 100, you will only be slashed 50).

- BondMinimum: A u32 determining the minimum amount of tokens that are acceptable to bond when submitting a post. Submissions with a bond lower than this amount will fail.
- VotingPeriod: A BlockNumber that determines the voting period of a post based on the block number the post was submitted at. Votes submitted after the period ends will fail. Once the period ends, the post can be resolved with `try_resolve_voting`.
- VoteMinimum: A u32 determining the minimum amount of tokens that are acceptable to vote with. Votes smaller than this value will fail.
- MaxVoters: A u32 determining the maximum amount of accounts that can vote on a post. This is used to bound a vector storing all of the accounts that have voted on a particular post, so performance may (assuming there are actual voters) linearly slow as the value is increased.
- StorageRent: A u32 determining the amount of tokens that must be locked in order to submit a post. This is separate from the post's bond and is not involved in the reward process. This value should be sufficiently high to prevent storage bloat attacks. The rent is unlocked once a post is ended, resolved, and removed from storage.
- MaxUrlLength: A u32 determining the maximum acceptable length of submitted URLs (in practice it could be any text/numbers/etc., this should be handled by the UI). The URLs are simply checked against this and then hashed, so this can be quite high in practice.
- UnfreezeLimit: A u32 determining the maximum number of accounts that can have their vote unfrozen when executing `try_end_post`. If the number of votes on a post exceeds this value, `try_end_post` will need to be called again.
