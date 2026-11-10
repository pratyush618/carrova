import type { About, PlanReview } from '../../engine/types';
import type { Computer } from '../../screens/shared/model';

/** This computer, as the engine describes it (`null` until it answered). */
export function thisComputer(about: About | null): Computer {
  return { name: about?.device_name ?? null, os: about?.os ?? null };
}

/** The computer a review's data comes from, as its manifest names it (unknown before the plan). */
export function sourceComputer(review: PlanReview | null): Computer {
  const source = review?.source ?? null;
  return { name: source?.device_name ?? null, os: source?.os ?? null };
}
