---
description: Create a plan for the specified task.
---

# How to create a plan

1. Ask the user for the task to create a plan for.
   It is important to ask for the public contracts to be considered. If the user does not provide the public contracts, suggest them based on the task description.
   As examples of things we understand as public contracts, but not limited to:

- Application services to add, modify or remove, and the methods signatures of each one of them.
- Domain events to add, modify or remove, and the attributes of each one of them.
- Test suites to add, modify or remove, and all the test cases inside each one of them.
- Database schemas to add, modify or remove, and the tables inside each one of them.
  IMPORTANT: Do not start creating the plan until the user has agreed on the specific contracts to be considered.
- UI Models to add, modify or remove, and the attributes of each one of them.

2. Save the plan in a new file in the `.agents/plans` directory.

# Plan sections

The plan should contain the following sections:

- Goal (brief description of the task)
- Context (important files, folders, and code to consider)
- Phases (IMPORTANT: each phase should be a vertical slice of the task)
- Description (brief description of the phase)
- To-do list (checkboxes list of actions to complete the phase)
- Verification (how to verify the phase is complete and correct)
- Documentation to update once all phases are completed
- Next step

# Considerations for each plan section

## Phases section

- Each one of the phases should be a vertical slice of the task. Avoid creating the endpoint controller in one phase and the service in another one. Split the task into as many phases as needed to make them easier to review and merge.
- We must be able to commit and push the code for each phase without breaking the build and the added code makes sense.
- Read the AGENTS.md file and the relevant documentation referenced in that file to understand the architecture and the coding conventions to follow while proposing the plan.
- Each phase is only completed once the verification step has been successfully passed. If not, we have to iterate the phase until it passes.

## Documentation section

- Once all phases are completed, we must update or create documentation related to what we have done.
- To know how and what to document reference the `./documentation/how-to-document.md` file

## Next step section

- Write it short and concise. It should be a single sentence that summarizes the next step to be taken to complete the task. That is, which phase should be completed next.

# User Input

$ARGUMENTS
