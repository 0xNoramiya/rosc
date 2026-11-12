// The band: woodblock, tuba, slide whistle, trombone and kazoo. Each sound
// is a short recording in public/sounds (see the manifest there for how each
// was made). Until a recording has loaded, or if it never does, the same cue
// is played by an oscillator instead, so the band is never silent. It plays
// only what the chain reports, and only after the visitor turns it on.

const SAMPLES = ['woodblock', 'tuba', 'sour', 'whistle', 'flatline', 'trombone', 'fanfare', 'balloon'] as const;
type Sample = (typeof SAMPLES)[number];

/// How loud each recording plays. They are levelled to one loudness, so
/// these only set the mix: the beat is quiet, the big moments are not.
const LEVEL: Record<Sample, number> = { woodblock: 0.5, tuba: 0.9, sour: 0.8, whistle: 0.8, flatline: 0.7, trombone: 0.8, fanfare: 0.9, balloon: 0.8 };

class Band {
  private ctx: AudioContext | null = null;
  private recordings = new Map<Sample, AudioBuffer>();
  private loading = false;
  on = false;

  enable(on: boolean) {
    this.on = on;
    if (!on) return;
    this.ctx ??= new AudioContext();
    void this.ctx.resume();
    this.load(this.ctx);
  }

  /// Fetches the recordings once. A missing or undecodable file is skipped
  /// and its cue stays on the oscillators.
  private load(ctx: AudioContext) {
    if (this.loading) return;
    this.loading = true;
    for (const name of SAMPLES) {
      fetch(`${import.meta.env.BASE_URL}sounds/${name}.mp3`)
        .then(response => (response.ok ? response.arrayBuffer() : Promise.reject(new Error(String(response.status)))))
        .then(bytes => ctx.decodeAudioData(bytes))
        .then(
          buffer => this.recordings.set(name, buffer),
          () => {},
        );
    }
  }

  /// Plays a recording `start` seconds from now. False if it is not loaded.
  private play(name: Sample, start = 0): boolean {
    const ctx = this.ctx;
    const buffer = this.recordings.get(name);
    if (!this.on || !ctx || !buffer) return false;
    const source = ctx.createBufferSource();
    const gain = ctx.createGain();
    source.buffer = buffer;
    gain.gain.value = LEVEL[name];
    source.connect(gain).connect(ctx.destination);
    source.start(ctx.currentTime + start);
    return true;
  }

  /// One oscillator note: a frequency that may slide, with a quick attack
  /// and a decay.
  private note(type: OscillatorType, from: number, to: number, start: number, length: number, level: number, vibrato = 0) {
    const ctx = this.ctx;
    if (!this.on || !ctx) return;
    const t = ctx.currentTime + start;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = type;
    osc.frequency.setValueAtTime(from, t);
    osc.frequency.exponentialRampToValueAtTime(to, t + length);
    if (vibrato) {
      const wobble = ctx.createOscillator();
      const depth = ctx.createGain();
      wobble.frequency.value = 28;
      depth.gain.value = vibrato;
      wobble.connect(depth).connect(osc.frequency);
      wobble.start(t);
      wobble.stop(t + length);
    }
    gain.gain.setValueAtTime(0.0001, t);
    gain.gain.exponentialRampToValueAtTime(level, t + 0.012);
    gain.gain.exponentialRampToValueAtTime(0.0001, t + length);
    osc.connect(gain).connect(ctx.destination);
    osc.start(t);
    osc.stop(t + length + 0.02);
  }

  /** A beat on the trace, or the metronome. */
  woodblock() {
    if (!this.play('woodblock')) this.note('square', 980, 620, 0, 0.06, 0.1);
  }
  /** A compression that counted. */
  tuba() {
    if (this.play('tuba')) return;
    this.note('sawtooth', 92, 82, 0, 0.22, 0.2);
    this.note('triangle', 46, 41, 0, 0.22, 0.25);
  }
  /** A press that missed. */
  sour() {
    if (this.play('sour')) return;
    this.note('sawtooth', 98, 70, 0, 0.3, 0.16);
    this.note('sawtooth', 104, 73, 0, 0.3, 0.12);
  }
  /** A breath. */
  whistle() {
    if (!this.play('whistle')) this.note('sine', 480, 1500, 0, 0.32, 0.12);
  }
  /** The heart stops: one long tone, then a slow sad trombone. */
  arrest() {
    if (!this.play('flatline')) this.note('sine', 880, 880, 0, 1.1, 0.08);
    if (this.play('trombone', 1.3)) return;
    [233, 220, 208].forEach((f, i) => this.note('sawtooth', f, f * 0.97, 1.3 + i * 0.42, 0.36, 0.14));
    this.note('sawtooth', 196, 150, 2.56, 1.1, 0.14, 4);
  }
  /** Return of spontaneous circulation: a kazoo fanfare. */
  fanfare() {
    if (this.play('fanfare')) return;
    [523, 659, 784, 1047, 784, 1047].forEach((f, i) => this.note('square', f, f, i * 0.13, i === 5 ? 0.5 : 0.12, 0.09, 14));
  }
  /** Time of death: a balloon letting go, then one triangle note. */
  deflate() {
    if (this.play('balloon')) return;
    this.note('sawtooth', 700, 90, 0, 1.4, 0.07, 30);
    this.note('sine', 2093, 2093, 1.6, 1.2, 0.08);
  }
}

export const band = new Band();
