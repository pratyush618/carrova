import { LiveFrame } from '../LiveFrame';
import { Shot } from '../Shot';

export function TryIt() {
  return (
    <section className="try" id="try" aria-labelledby="try-title">
      <div className="try-copy">
        <h2 id="try-title" className="section-title">
          Try a move. Right here.
        </h2>
        <p className="section-sub">
          This is Carrova's own receive screen, running in your browser on sample data. Answer its
          three questions, then approve.
        </p>
      </div>
      <div className="window try-live">
        <LiveFrame
          scene="receive"
          title="Try Carrova's receive screen: answer, approve, watch it move"
        />
      </div>
      <div className="window try-still">
        <Shot
          name="review"
          sizes="calc(100vw - 32px)"
          alt="The plan before anything moves: 412 new files, 3 replaced and backed up first, and an Approve and move button."
        />
      </div>
      <p className="fine try-note">
        Nothing here touches your computer.{' '}
        <span className="try-still">Try it on a bigger screen.</span>
      </p>
    </section>
  );
}
