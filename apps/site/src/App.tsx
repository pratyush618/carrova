import { useRelease } from './download/useRelease';
import { Apps } from './sections/Apps';
import { Download } from './sections/Download';
import { Footer } from './sections/Footer';
import { Hero } from './sections/Hero';
import { Nav } from './sections/Nav';
import { Phone } from './sections/Phone';
import { Privacy } from './sections/Privacy';
import { Safety } from './sections/Safety';
import { Statement } from './sections/Statement';
import { Steps } from './sections/Steps';

export function App() {
  const release = useRelease();
  return (
    <>
      <a className="skip" href="#main">
        Skip to content
      </a>
      <Nav />
      <main id="main">
        <Hero release={release} />
        <Statement />
        <Steps />
        <Safety />
        <Privacy />
        <Apps />
        <Phone />
        <Download release={release} />
      </main>
      <Footer version={release?.version} />
    </>
  );
}
