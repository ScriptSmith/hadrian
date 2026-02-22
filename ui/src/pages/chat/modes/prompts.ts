/**
 * Default prompt templates for conversation modes
 */

/**
 * Default routing prompt template for routed mode.
 * The router model analyzes the prompt and selects the best model.
 */
export const DEFAULT_ROUTING_PROMPT = `You are a routing assistant. Analyze the user's message and select the most appropriate model to handle it.

Available models:
{models}

Based on the user's message, respond with ONLY the model identifier (exactly as shown above) that would be best suited to answer. Consider:
- Coding questions → prefer models known for code (Claude, GPT-4)
- Creative writing → prefer models good at creativity
- Math/reasoning → prefer models strong in logic
- General questions → any model works

Respond with just the model identifier, nothing else.`;

/**
 * Default synthesis prompt template for synthesized mode.
 * The synthesizer model combines responses from multiple models into a unified answer.
 */
export const DEFAULT_SYNTHESIS_PROMPT = `You are a synthesis assistant. You have received responses from multiple AI models to the same question. Your task is to create a single, comprehensive response that:

1. Identifies the key insights and valuable information from each response
2. Resolves any contradictions by favoring the most accurate/well-reasoned answer
3. Combines complementary information into a coherent whole
4. Maintains accuracy and doesn't introduce new information not present in the source responses
5. Presents the synthesized answer in a clear, well-organized format

Here are the responses from different models:

{responses}

Please synthesize these responses into a single, comprehensive answer. Do not mention that you are synthesizing or reference the individual models - just provide the best unified answer.`;

/**
 * Default refinement prompt template for refined mode.
 * Each model refines/improves the previous response.
 */
export const DEFAULT_REFINEMENT_PROMPT = `You are refining a previous response. Your task is to improve it by:

1. Correcting any errors or inaccuracies
2. Adding missing information or context
3. Improving clarity and organization
4. Making the response more comprehensive while staying concise
5. Enhancing the quality of explanations or examples

Here is the previous response to refine:

{previous_response}

Please provide an improved version of this response. Do not mention that you are refining or reference the previous response - just provide the best possible answer.`;

/**
 * Default critique prompt template for critiqued mode.
 * Critic models analyze and provide feedback on the initial response.
 */
export const DEFAULT_CRITIQUE_PROMPT = `You are a critical reviewer. Analyze the following response and provide constructive feedback to help improve it.

Your critique should:
1. Identify any errors, inaccuracies, or misleading information
2. Point out gaps or missing information
3. Suggest improvements to clarity and structure
4. Highlight both strengths and weaknesses
5. Be specific and actionable

Here is the response to critique:

{response}

Provide your critique in a structured format. Be thorough but concise.`;

/**
 * Default revision prompt template for critiqued mode.
 * The primary model revises its response based on critiques received.
 */
export const DEFAULT_REVISION_PROMPT = `You previously provided a response that received critiques from other reviewers. Your task is to revise your response by incorporating the valid feedback while maintaining your original insights.

Your original response:
{original_response}

Critiques received:
{critiques}

Please provide a revised response that:
1. Addresses the valid points raised in the critiques
2. Corrects any identified errors
3. Adds any missing information mentioned
4. Improves clarity and structure as suggested
5. Maintains the strengths of your original response

Do not mention that this is a revised response - just provide the best possible answer.`;

/**
 * Default voting prompt template for elected mode.
 * Each model evaluates all candidate responses and votes for the best one.
 */
export const DEFAULT_VOTING_PROMPT = `You are a judge evaluating responses to a user's question. Your task is to vote for the BEST response among the candidates.

User's original question:
{question}

Here are the candidate responses to evaluate:

{candidates}

Evaluate each response based on:
1. Accuracy - Is the information correct and reliable?
2. Completeness - Does it fully address the question?
3. Clarity - Is it well-organized and easy to understand?
4. Helpfulness - Does it provide actionable and useful information?
5. Relevance - Does it stay focused on the question asked?

IMPORTANT: You MUST respond with ONLY the candidate number (1, 2, 3, etc.) of the response you vote for. Do not include any other text, explanation, or reasoning - just the number.

Your vote:`;

/**
 * Default tournament judging prompt template for tournament mode.
 * A judge model compares two responses and selects a winner.
 */
export const DEFAULT_TOURNAMENT_JUDGING_PROMPT = `You are a judge in a tournament. Your task is to compare two responses to a user's question and select the BETTER response.

User's original question:
{question}

Here are the two competing responses:

--- Response A ---
{response_a}

--- Response B ---
{response_b}

Evaluate each response based on:
1. Accuracy - Is the information correct and reliable?
2. Completeness - Does it fully address the question?
3. Clarity - Is it well-organized and easy to understand?
4. Helpfulness - Does it provide actionable and useful information?
5. Relevance - Does it stay focused on the question asked?

IMPORTANT: You MUST respond with ONLY the letter "A" or "B" to indicate which response wins. Do not include any other text, explanation, or reasoning - just the single letter.

Winner:`;

/**
 * Default consensus prompt template for consensus mode.
 * Each model revises their response based on seeing all other responses.
 */
export const DEFAULT_CONSENSUS_PROMPT = `You are participating in a consensus-building process. Multiple AI models have provided responses to the same question. Your task is to revise your response by considering all perspectives and working toward agreement.

User's original question:
{question}

Here are the current responses from all participants:

{responses}

Please provide a revised response that:
1. Incorporates the strongest points from all responses
2. Addresses any contradictions by choosing the most accurate position
3. Moves toward a consensus view while maintaining accuracy
4. Preserves unique valuable insights that others may have missed
5. Aims for clarity and completeness

Do not mention that you are revising or reference the other responses - just provide your best unified answer.`;

/**
 * Default debate opening prompt template for debated mode.
 * Each model presents their opening argument from their assigned position.
 */
export const DEFAULT_DEBATE_OPENING_PROMPT = `You are participating in a structured debate. You have been assigned the "{position}" position on the following question:

{question}

Present your opening argument from the {position} perspective. Your argument should:
1. Clearly state your position
2. Provide compelling evidence and reasoning
3. Anticipate potential counterarguments
4. Be persuasive but intellectually honest

Present your opening argument now.`;

/**
 * Default debate rebuttal prompt template for debated mode.
 * Each model responds to the opposing arguments.
 */
export const DEFAULT_DEBATE_REBUTTAL_PROMPT = `You are participating in a structured debate. You are arguing the "{position}" position.

The original question was:
{question}

Here are the arguments from the previous round:

{arguments}

Now provide your rebuttal. Your rebuttal should:
1. Address the strongest points made by the opposing side(s)
2. Defend your position against criticisms
3. Introduce new supporting evidence if available
4. Strengthen your overall argument

Provide your rebuttal now.`;

/**
 * Default debate summary prompt template for debated mode.
 * The summarizer model synthesizes the debate into a balanced conclusion.
 */
export const DEFAULT_DEBATE_SUMMARY_PROMPT = `You observed a structured debate on the following question:

{question}

Here is the complete debate transcript:

{debate}

Synthesize this debate into a balanced summary that:
1. Fairly represents all positions presented
2. Identifies the strongest arguments from each side
3. Notes where positions found common ground
4. Highlights unresolved points of contention
5. Provides a nuanced conclusion that acknowledges the complexity
6. Judges the merits of each position based on evidence and arguments, and picks the best one

Do not mention that you are summarizing a debate - just provide a thoughtful, balanced analysis.`;

/**
 * Default council opening prompt template for council mode.
 * Each model presents their initial perspective from their assigned role.
 */
export const DEFAULT_COUNCIL_OPENING_PROMPT = `You are participating in a council discussion. You have been assigned the role of "{role}" - you should approach all topics from this perspective.

The topic under discussion is:
{question}

Present your initial perspective on this topic from your assigned role. Your contribution should:
1. Clearly state your perspective based on your role
2. Provide relevant insights and considerations specific to your expertise
3. Identify key concerns or opportunities from your vantage point
4. Be constructive and collaborative in tone

Present your perspective now.`;

/**
 * Default council discussion prompt template for council mode.
 * Each model responds to other perspectives in the discussion.
 */
export const DEFAULT_COUNCIL_DISCUSSION_PROMPT = `You are participating in a council discussion as "{role}".

The topic under discussion is:
{question}

Here are the perspectives shared so far:

{perspectives}

Now respond to the other council members from your role's perspective. Your response should:
1. Acknowledge valuable insights from other perspectives
2. Build on compatible ideas while addressing potential conflicts
3. Offer additional considerations from your unique viewpoint
4. Work toward finding common ground or complementary approaches

Provide your response now.`;

/**
 * Default council synthesis prompt template for council mode.
 * The synthesizer model combines all perspectives into a comprehensive response.
 */
export const DEFAULT_COUNCIL_SYNTHESIS_PROMPT = `You observed a council discussion where multiple experts with different roles discussed:

{question}

Here is the complete discussion:

{discussion}

Synthesize this discussion into a comprehensive response that:
1. Integrates the valuable insights from each perspective
2. Acknowledges trade-offs and different priorities
3. Provides a balanced recommendation or answer that considers all viewpoints
4. Identifies areas of agreement and remaining considerations
5. Presents a coherent, actionable conclusion

Do not mention that you are synthesizing a council discussion - just provide a thoughtful, integrated answer.`;

/**
 * Default council role assignment prompt template for council mode.
 * Used when auto-assign roles is enabled - the synthesizer assigns roles to council members.
 */
export const DEFAULT_COUNCIL_ROLE_ASSIGNMENT_PROMPT = `You are setting up a council discussion to address the following question:

{question}

You have {count} council members who will discuss this topic from different perspectives. Assign each one a unique role/perspective that would be valuable for addressing this question.

The council members are:
{members}

Respond with ONLY a JSON object mapping each model to their assigned role. Choose roles that are relevant to the specific question being discussed. Be creative and specific.

Example format:
{
  "model-name-1": "Role for first model",
  "model-name-2": "Role for second model"
}

Respond with only the JSON object, no other text.`;

/**
 * Default hierarchical decomposition prompt template for hierarchical mode.
 * The coordinator model breaks down the task into subtasks for workers.
 */
export const DEFAULT_HIERARCHICAL_DECOMPOSITION_PROMPT = `You are a task coordinator. Analyze the following question/task and break it down into discrete subtasks that can be delegated to specialized workers.

Question/Task:
{question}

You have {count} worker models available:
{workers}

Break this task into subtasks that can be worked on independently. Each subtask should be:
1. Self-contained and clearly defined
2. Assigned to the most appropriate worker model
3. Completable without dependencies on other subtasks (if possible)

Respond with ONLY a JSON object in this exact format:
{
  "subtasks": [
    {
      "id": "subtask-1",
      "description": "Clear description of what this subtask involves",
      "assignedModel": "model-name"
    }
  ]
}

Create between 2 and {count} subtasks (one per worker at most). Be specific about what each worker should do.

Respond with only the JSON object, no other text.`;

/**
 * Default hierarchical worker prompt template for hierarchical mode.
 * Workers complete their assigned subtasks.
 */
export const DEFAULT_HIERARCHICAL_WORKER_PROMPT = `You have been assigned a specific subtask by a coordinator.

Overall context:
{context}

Your assigned task:
{task}

Complete this specific task thoroughly and accurately. Focus only on your assigned task and provide a comprehensive response. Your work will be combined with other workers' results to form a complete answer.`;

/**
 * Default hierarchical synthesis prompt template for hierarchical mode.
 * The coordinator combines all worker results into a final response.
 */
export const DEFAULT_HIERARCHICAL_SYNTHESIS_PROMPT = `You previously broke down a task into subtasks and delegated them to worker models. The workers have completed their subtasks.

Original question/task:
{question}

Here are the results from each worker:

{results}

Synthesize these results into a single, comprehensive response that:
1. Integrates all the subtask results coherently
2. Ensures the complete original question is fully answered
3. Resolves any inconsistencies between worker outputs
4. Presents the information in a logical, well-organized manner
5. Adds any connecting context needed for flow

Do not mention the subtasks, workers, or delegation process - just provide a polished, unified answer.`;

/**
 * Default explainer initial prompt template for explainer mode.
 * The first model explains the topic at the specified audience level.
 */
export const DEFAULT_EXPLAINER_INITIAL_PROMPT = `You are explaining a topic to a {level} audience. Your explanation should be tailored to this specific knowledge level.

The topic/question to explain:
{question}

Guidelines for {level} audience:
{level_guidelines}

Provide a clear, comprehensive explanation appropriate for this audience level. Do not mention who you are explaining to - just provide the explanation directly.`;

/**
 * Default explainer simplification prompt template for explainer mode.
 * Subsequent models simplify or adapt the explanation for different audience levels.
 */
export const DEFAULT_EXPLAINER_SIMPLIFY_PROMPT = `You are adapting an explanation to a {level} audience. You have access to a more detailed/technical explanation that needs to be made accessible to this audience.

The original question:
{question}

The previous explanation:
{previous_explanation}

Guidelines for {level} audience:
{level_guidelines}

Transform this explanation to be appropriate for a {level} audience:
- Adjust vocabulary and complexity
- Add helpful analogies or examples if needed
- Remove or explain technical jargon
- Maintain accuracy while improving accessibility
- Keep it engaging and clear

Do not mention that you are simplifying or reference the previous explanation - just provide the adapted explanation directly.`;

/**
 * Default audience level guidelines for explainer mode.
 * Maps audience levels to specific guidance for the model.
 */
export const DEFAULT_AUDIENCE_GUIDELINES: Record<string, string> = {
  expert: `- Use precise technical terminology
- Assume deep domain knowledge
- Focus on nuances, edge cases, and advanced considerations
- Reference relevant research, standards, or best practices
- Be concise - experts appreciate density over verbosity`,
  intermediate: `- Use standard technical terms but explain specialized ones
- Assume working knowledge of the field
- Provide context for complex concepts
- Include practical examples and applications
- Balance depth with accessibility`,
  beginner: `- Use simple, everyday language
- Avoid jargon or define it immediately when necessary
- Use analogies to familiar concepts
- Build understanding step by step
- Include concrete, relatable examples
- Be patient and thorough`,
  child: `- Use very simple words and short sentences
- Use fun analogies to things kids know (games, animals, toys)
- Make it engaging and interesting
- Break complex ideas into tiny steps
- Use lots of examples from everyday life
- Be encouraging and friendly`,
  "non-technical": `- Avoid all technical jargon
- Focus on practical implications and real-world relevance
- Use analogies from everyday life
- Emphasize the "so what" and practical takeaways
- Keep it engaging and relevant to daily life`,
};

/**
 * Default confidence-weighted response prompt template.
 * Instructs the model to provide both a response AND a confidence score.
 */
export const DEFAULT_CONFIDENCE_RESPONSE_PROMPT = `Answer the following question thoughtfully and completely.

After your response, on a new line, provide a confidence score for your answer using this exact format:
CONFIDENCE: [score]

Where [score] is a decimal number between 0.0 and 1.0 representing how confident you are in your answer:
- 1.0 = Absolutely certain, well-established fact
- 0.8-0.9 = Highly confident, strong evidence
- 0.6-0.7 = Moderately confident, reasonable certainty
- 0.4-0.5 = Uncertain, could go either way
- 0.2-0.3 = Low confidence, speculative
- 0.0-0.1 = Very uncertain, mostly guessing

Be honest about your confidence level. Consider:
- How well-established is the information?
- Is there potential for ambiguity or multiple interpretations?
- Are you extrapolating beyond your training data?

Question:
{question}`;

/**
 * Default confidence-weighted synthesis prompt template.
 * Instructs the synthesizer to combine responses weighted by confidence.
 */
export const DEFAULT_CONFIDENCE_SYNTHESIS_PROMPT = `You are synthesizing multiple AI responses, each with a self-assessed confidence score. Your task is to create a unified answer that weighs each response according to its confidence level.

Here are the responses with their confidence scores:

{responses}

Guidelines for synthesis:
1. Give more weight to responses with higher confidence scores
2. For conflicting information, prefer the higher-confidence source unless there's a clear error
3. If a low-confidence response contains unique valuable information, include it but note the uncertainty
4. The final answer should reflect the weighted consensus of the responses
5. If all responses have low confidence, acknowledge the uncertainty in your synthesis

Create a comprehensive, well-organized response that represents the confidence-weighted synthesis of these answers. Do not mention the confidence scores or that you are synthesizing - just provide the best unified answer.`;
