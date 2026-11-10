import { historyTimeline } from '../history/fixtures';
import { NOW } from '../shared/fixtures';
import type { HomeModel } from './model';

export const homeReturning: HomeModel = {
  now: NOW,
  recent: historyTimeline.entries.slice(0, 2),
  askReports: false,
};

/** A fresh install: no moves yet, error-report consent not answered. */
export const homeFirst: HomeModel = { now: NOW, recent: [], askReports: true };
