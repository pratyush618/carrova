import { useEffect, useState } from 'react';
import { archFromHints, detectArch, type Arch } from './platform';

interface UaData {
  getHighEntropyValues(hints: string[]): Promise<{ architecture?: string }>;
}

const uaData = (navigator as Navigator & { userAgentData?: UaData }).userAgentData;

/**
 * The visitor's CPU architecture, or undefined when it can't be verified.
 * Chromium freezes its user agent to "Linux x86_64" on every Linux, so there
 * only Client Hints count; other browsers report the real arch in the UA.
 */
export function useVisitorArch(): Arch | undefined {
  const [arch, setArch] = useState<Arch | undefined>(() =>
    uaData ? undefined : detectArch(navigator.userAgent, navigator.platform),
  );
  useEffect(() => {
    uaData
      ?.getHighEntropyValues(['architecture'])
      .then((v) => {
        setArch(archFromHints(v.architecture ?? ''));
      })
      .catch(() => undefined);
  }, []);
  return arch;
}
