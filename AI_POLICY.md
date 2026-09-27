# cargo-mutants AI Policy

`cargo-mutants` has adopted the [LLM Usage Policy](https://forge.rust-lang.org/policies/llm-usage.html) used by `rust-lang/rust` and some other Rust projects. Read that page for more details and edge cases.

**Please do not post LLM-generated issues, PRs, or comments.**

This applies even if you have reviewed the generated content yourself, or are willing to take responsibility.

# Rationale

This is a personal-time non-commercial project. Although I appreciate the support from GitHub sponsors and want to make it useful for the Rust community at large, it is primarily fueled by my personal enjoyment and enthusiasm for the project. Maintainer bandwidth is the limiting reagent.

1. I simply don't enjoy reviewing LLM-generated PRs or comments, even the "good ones", and I find that this is sapping my energy to engage with the PR queue or project at all.

2. In 2026 I've seen a significant number of low-effort, low-quality, automated LLM issues and PRs, in some cases by bots that are apparently trying to file as many PRs as possible, without in any sense "caring" about the results. It can take significant effort to perceive that the content is actually incoherent. Although humans can also generate low-quality content there is now a difference in scale. Although this shouldn't necessarily count against the "good" AI content, it shows that a policy of only asking people/agents to be responsible is not sufficient to defend my time.

3. Large volumes of LLM-generated content push developers towards tuning out and trusting the agents to make decisions. That may well be a good strategy for some projects but I simply don't want to do that here. I want to understand and be involved in shaping the code.

4. I think there is just very little point in someone else prompting an agent to do a thing. If I want an LLM to implement a change, I can prompt it myself:

   * I can have a back-and-forth with the agent more effectively and directly myself rather than through a meat proxy and the GitHub review UI.
   * Engaging directly makes it relatively easier to maintain an understanding of the code than seeing only the final result.

I am not claiming this is the right policy for every project for all time, but it seems like the right one at this particular time and place.
