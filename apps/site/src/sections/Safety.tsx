import { Icon } from '../Icon';
import { Shot } from '../Shot';

export function Safety() {
  return (
    <section className="safety" aria-label="Safe by default">
      <article className="tile">
        <div className="tile-copy">
          <h2>Nothing changes until you say so.</h2>
          <p>
            Your new computer shows the whole plan first, and asks about anything sensitive. Every
            file it would replace is backed up before it's touched.
          </p>
        </div>
        <div className="tile-shot crop crop-decide">
          <Shot
            name="decide"
            sizes="(min-width: 1360px) 900px, 120vw"
            alt="Needs your answer: bring your SSH keys or leave them behind, the same for GnuPG keys, and close VS Code or skip it."
          />
        </div>
      </article>
      <article className="tile">
        <div className="tile-copy">
          <h2>
            Changed your mind? <span className="nowrap">Undo.</span>
          </h2>
          <p>
            Undo puts back everything a move replaced, and leaves alone any file you've edited
            since.
          </p>
          <p className="tile-note">
            <Icon name="undo" /> Every move, kept in History.
          </p>
        </div>
        <div className="tile-shot crop crop-undo">
          <Shot
            name="undo"
            sizes="(min-width: 1360px) 1300px, 240vw"
            alt="Undo this move? The computer goes back to how it was yesterday: 412 files removed, 3 put back, 1 file you changed since kept as a copy."
          />
        </div>
      </article>
    </section>
  );
}
