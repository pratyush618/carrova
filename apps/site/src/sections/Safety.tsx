import { Icon } from '../Icon';
import { doneShot, reviewShot } from '../shots';

export function Safety() {
  return (
    <section className="safety" aria-label="Safe by default">
      <article className="tile">
        <div className="tile-copy">
          <h2>Nothing changes until you say so.</h2>
          <p>
            Your new computer shows the whole plan first. Every file it would replace is backed up
            before it's touched.
          </p>
        </div>
        <div className="tile-shot">
          <img
            src={reviewShot}
            width="2200"
            height="1440"
            loading="lazy"
            alt="The plan: files to add, files to replace with backups, and space needed."
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
        <div className="tile-shot">
          <img
            src={doneShot}
            width="2200"
            height="1440"
            loading="lazy"
            alt="A finished move with its summary."
          />
        </div>
      </article>
    </section>
  );
}
