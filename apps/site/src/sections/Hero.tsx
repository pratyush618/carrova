import { DownloadButton } from '../download/DownloadButton';
import type { Release } from '../download/useRelease';
import { homeShot } from '../shots';

export function Hero({ release }: { readonly release: Release | undefined }) {
  return (
    <section className="hero" id="top">
      <h1>
        Your setup.
        <br />
        On your new computer.
      </h1>
      <p className="hero-sub">
        Carrova moves your app settings, keys and data from your old computer to your new one, over
        your own network. You choose what comes along.
      </p>
      <div className="hero-cta">
        <DownloadButton release={release} />
        <a className="more" href="#how">
          See how it works
        </a>
      </div>
      <p className="fine">Free and open source. No account. No cloud.</p>
      <figure className="hero-shot">
        <div className="window">
          <img
            src={homeShot}
            width="2200"
            height="1440"
            alt="Carrova's home screen: Send from this computer, or Receive on this computer, with recent moves below."
          />
        </div>
      </figure>
    </section>
  );
}
