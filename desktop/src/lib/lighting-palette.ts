/** Static LCDV2 palette slots from Galahad2VisionProfile (Windows 2.1.28). */
export function lightingColorCount(kind: 'pump' | 'fan', mode: string): number | null {
  if (mode === 'static_color') mode = 'static';
  if (kind === 'fan' && !['rainbow','rainbow_morph','static','breathing','runway','meteor'].includes(mode)) return null;
  switch (mode) {
    case 'rainbow': case 'rainbow_morph': case 'colorful_starry_night': case 'colors_morph': return 0;
    case 'static': case 'breathing': case 'static_starry_night': case 'voice': case 'burst': return 1;
    case 'runway': case 'transmit': return 2;
    case 'meteor': case 'ticker_tape': case 'fluctuation': case 'big_bang': case 'bounce': return 4;
    default: return null;
  }
}
