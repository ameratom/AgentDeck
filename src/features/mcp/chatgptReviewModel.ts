import type { ChatgptReviewCheck, ChatgptReviewHealth } from "../../lib/types";

export function reviewReadyLabel(health: ChatgptReviewHealth): string {
  return health.readyForReviewers ? "Ready for reviewers" : "Action needed";
}

export function reviewReadyClass(health: ChatgptReviewHealth): string {
  return health.readyForReviewers
    ? "chatgpt-review-state ready"
    : "chatgpt-review-state pending";
}

export function reviewCheckClass(check: ChatgptReviewCheck): string {
  return check.passed ? "review-check passed" : "review-check failed";
}

export function operationalChecks(health: ChatgptReviewHealth): ChatgptReviewCheck[] {
  return health.checks.filter(
    (check) => check.id !== "platform-review" && check.id !== "publish-gate",
  );
}