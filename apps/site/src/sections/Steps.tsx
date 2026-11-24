import { useEffect, useRef, useState } from 'react';
import { doneShot, reviewShot, sendShot } from '../shots';

const STEPS = [
  {
    title: 'Pick what moves.',
    body: 'On your old computer, Carrova finds your apps and suggests what to bring. Change any of it. Then it shows a short pairing code.',
    shot: sendShot,
    alt: 'Send screen: a list of found apps, each with a checkbox and its size.',
  },
  {
    title: 'See the plan first.',
    body: 'Type the code on your new computer. Before anything is written, you see what will be added, what will be replaced and backed up, and the space it needs.',
    shot: reviewShot,
    alt: 'Review screen: new files, replaced files with backups, and space needed, with an Approve button.',
  },
  {
    title: 'Arrive.',
    body: 'Your settings land in place. Carrova lists the apps to install, and they open already set up the way you left them.',
    shot: doneShot,
    alt: 'Done screen: the move is complete, with the apps to install listed.',
  },
] as const;

/** Text scrolls by while the screenshot stays put and follows the step in view. */
export function Steps() {
  const [active, setActive] = useState(0);
  const refs = useRef<(HTMLElement | null)[]>([]);

  useEffect(() => {
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) setActive(Number((e.target as HTMLElement).dataset['step']));
        }
      },
      { rootMargin: '-45% 0px -45% 0px' },
    );
    for (const el of refs.current) if (el) io.observe(el);
    return () => {
      io.disconnect();
    };
  }, []);

  return (
    <section className="steps" id="how" aria-labelledby="how-title">
      <h2 id="how-title" className="section-title">
        Three steps. One evening, tops.
      </h2>
      <div className="steps-grid">
        <ol className="steps-text">
          {STEPS.map((s, i) => (
            <li
              key={s.title}
              data-step={i}
              ref={(el) => {
                refs.current[i] = el;
              }}
              className={i === active ? 'is-active' : undefined}
            >
              <h3>{s.title}</h3>
              <p>{s.body}</p>
              <div className="window steps-inline">
                <img src={s.shot} width="2200" height="1440" loading="lazy" alt={s.alt} />
              </div>
            </li>
          ))}
        </ol>
        <div className="steps-stage" aria-hidden="true">
          <div className="window">
            {STEPS.map((s, i) => (
              <img
                key={s.title}
                src={s.shot}
                width="2200"
                height="1440"
                loading="lazy"
                alt=""
                className={i === active ? 'is-active' : undefined}
              />
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
