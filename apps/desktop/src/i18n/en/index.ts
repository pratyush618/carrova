import { apps } from './apps';
import { calls } from './calls';
import { common } from './common';
import { ecosystem } from './ecosystem';
import { errors } from './errors';
import { file } from './file';
import { history } from './history';
import { mobile } from './mobile';
import { nearby } from './nearby';
import { override } from './override';
import { receive } from './receive';
import { send } from './send';
import { settings } from './settings';
import { start } from './start';
import { transfer } from './transfer';
import { undo } from './undo';

/** The English catalogue: the source of every user-facing string and of `t()` key types. */
export const en = {
  ...common,
  errors,
  start,
  send,
  transfer,
  receive,
  file,
  undo,
  history,
  nearby,
  apps,
  settings,
  override,
  ecosystem,
  mobile,
  calls,
} as const;
