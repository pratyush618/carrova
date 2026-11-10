import { NOW, SENDER, YESTERDAY } from '../shared/fixtures';
import type { HomeModel, WelcomeModel } from './model';

export const welcome: WelcomeModel = { errorReports: false };

export const homeReturning: HomeModel = {
  now: NOW,
  last: { direction: 'received', peer: SENDER, when: YESTERDAY, apps: 5, undoable: true },
};

export const homeFirst: HomeModel = { now: NOW, last: null };
