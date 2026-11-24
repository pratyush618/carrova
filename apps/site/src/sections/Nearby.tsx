import { LiveFrame } from '../LiveFrame';
import { Shot } from '../Shot';

export function Nearby() {
  return (
    <section className="nearby" aria-labelledby="nearby-title">
      <div className="nearby-copy">
        <h2 id="nearby-title" className="section-title">
          Your other computers, already there.
        </h2>
        <p className="section-sub">
          Open Nearby and Carrova shows what's on your network. Computers you've paired before send
          without a code, and every move is still reviewed on the one receiving it.
        </p>
      </div>
      <div className="window nearby-live">
        <LiveFrame
          scene="nearby"
          title="Carrova's Nearby map, live: open a device to see what you can do"
        />
      </div>
      <div className="window nearby-shot">
        <Shot
          name="nearby"
          sizes="(min-width: 1360px) 1200px, calc(100vw - 32px)"
          alt="Nearby: this computer in the middle, with Desk PC, Ana's laptop and a Pixel 6 close by, and older computers marked away."
        />
      </div>
    </section>
  );
}
