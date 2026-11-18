"""Gera os samples estéreo (ogg q6) por instrumento + manifest.json (os de assets/samples/).

Uso: python3 tools/extract-samples.py <pasta de saída>
Precisa de ffmpeg e numpy, e das gravações-fonte baixadas ao lado do script
(SCC em tools/scc/Taiko-Drums-main/; as do freesound no caminho usado abaixo).
Não faz parte do app: os .ogg resultantes ficam versionados em assets/samples/.

Fontes:
  SCC  = SCC Taiko Drums (floe-audio/Taiko-Drums), CC BY-SA 4.0
  TS1  = taikoset1_ste58 (whatsanickname4u, freesound 127337), CC BY 3.0
  KANE = Japan_Kashiwa_Shrine_Hitting_a_Small_Gong (RutgerMuller, freesound 364847), CC0
"""
import json, os, subprocess, sys
import numpy as np

SCR = os.path.dirname(os.path.abspath(__file__))
SCC = os.path.join(SCR, 'scc/Taiko-Drums-main/library/Samples')
OUT = sys.argv[1]
SR = 48000
os.makedirs(OUT, exist_ok=True)


def load(path):
    raw = subprocess.run(['ffmpeg', '-v', 'error', '-i', path, '-ac', '2', '-f', 'f32le', '-ar', str(SR), '-'],
                         capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype=np.float32).reshape(-1, 2).copy()


def save(name, x):
    p = os.path.join(OUT, name + '.ogg')
    subprocess.run(['ffmpeg', '-v', 'error', '-y', '-f', 'f32le', '-ar', str(SR), '-ac', '2', '-i', '-',
                    '-c:a', 'libvorbis', '-q:a', '6', p], input=x.astype(np.float32).tobytes(), check=True)
    return name


def trim(x, max_sec, fade=0.06, pre=0.0):
    """corta silêncio inicial (mantém `pre` s) e limita a cauda com fade-out"""
    mono = np.abs(x).max(1)
    pk = mono.max()
    i0 = max(0, int(np.argmax(mono > pk * 0.05)) - int(pre * SR))
    x = x[i0:]
    # fim natural: último ponto acima de -60 dB do pico
    above = np.nonzero(np.abs(x).max(1) > pk * 1e-3)[0]
    end = min(len(x), (above.max() if len(above) else len(x)) + int(0.05 * SR), int(max_sec * SR))
    x = x[:end].copy()
    nf = min(int(fade * SR), len(x) // 2)
    x[-nf:] *= np.linspace(1, 0, nf)[:, None] ** 2
    return x


def peak(x):
    return float(np.abs(x).max())


manifest = {}
report = []


def add(inst, zone, layers, gains=None):
    manifest.setdefault(inst, {})[zone] = layers


# ---------- SCC: layers conforme o SFZ original ----------
def scc_layers(inst, zone, spec, max_sec):
    """spec = lista de layers (suave→forte), cada uma lista de arquivos SCC (RR)"""
    xs = [[trim(load(os.path.join(SCC, f + '.flac')), max_sec) for f in layer] for layer in spec]
    # normaliza o conjunto inteiro pelo pico máximo (preserva a dinâmica natural entre layers)
    g = 0.89 / max(peak(x) for layer in xs for x in layer)
    out = []
    for li, layer in enumerate(xs):
        names = []
        for ri, x in enumerate(layer):
            names.append(save(f'{inst}-{zone}-{li}-{ri}', x * g))
        out.append(names)
    add(inst, zone, out)
    report.append(f'{inst}.{zone}: SCC ' + ' | '.join(','.join(l) for l in spec))


STICKS = [['Taiko Drum Sticks-1', 'Taiko Drum Sticks-0'],
          ['Taiko Drum Sticks-2', 'Taiko Drum Sticks-3', 'Taiko Drum Sticks-5'],
          ['Taiko Drum Sticks-4', 'Taiko Drum Sticks-6']]

scc_layers('chu', 'head', [['taiko1-p', 'taiko1-m'], ['taiko1-f1', 'taiko1-f2']], 2.5)
scc_layers('chu', 'rim', STICKS, 2.0)
scc_layers('nagado', 'head', [['Taiko Drum Hit 1-15', 'Taiko Drum Hit 1-16'],
                              ['Taiko Drum Hit 1-27', 'Taiko Drum Hit 1-28', 'Taiko Drum Hit 2-6']], 3.5)
scc_layers('onagado', 'head', [['Taiko Drum Hit 2-2', 'Taiko Drum Hit 1-25'],
                               ['Taiko Drum Hit 1-18', 'Taiko Drum Hit 2-7'],
                               ['Taiko Drum Hit 2-5']], 4.0)
scc_layers('odaiko', 'head', [['Taiko Drum Hit 3-4', 'Taiko Drum Hit 3-3'], ['Taiko Drum Hit 3-2'],
                              ['Taiko Drum Hit 3-1'], ['Taiko Drum Hit 3-0']], 5.0)
scc_layers('shime', 'rim', [['Taiko Lt Sticks-0', 'Taiko Lt Sticks-1'], ['Taiko Lt Sticks-2'],
                            ['Taiko Lt Sticks-3', 'Taiko Lt Sticks-4']], 0.8)

# ---------- TS1: golpes isolados do taikoset (sala real) ----------
ts = load(os.path.join(SCR, 'fs/taikoset1.mp3'))
mono = ts.mean(1)
hop = 240
n = len(mono) // hop
env = np.sqrt((mono[:n * hop].reshape(n, hop) ** 2).mean(1))
db = 20 * np.log10(env + 1e-9)
flo = np.percentile(db, 20)
ons = []
last = -1e9
for i in range(4, n):
    if db[i] - db[i - 4] > 12 and db[i] > flo + 20 and (i - last) * hop / SR > 0.12:
        ons.append(i + int(np.argmax(db[i:i + 10])))
        last = i
hits = []
for k, j in enumerate(ons):
    t = j * hop / SR
    nxt = ons[k + 1] * hop / SR if k + 1 < len(ons) else t + 3
    a = int((t - 0.012) * SR)
    seg = mono[a + int(0.015 * SR): a + int(min(0.3, nxt - t) * SR)]
    sp = np.abs(np.fft.rfft(seg * np.hanning(len(seg)), 1 << 15))
    fr = np.fft.rfftfreq(1 << 15, 1 / SR)
    hits.append(dict(t=t, gap=nxt - t, lvl=float(db[j]), bright=float((sp * fr).sum() / sp.sum()), a=a))


def ts_layers(inst, zone, t0, t1, min_gap, max_sec, cond=lambda h: True, nlayers=3, max_rr=4):
    sel = [h for h in hits if t0 <= h['t'] < t1 and h['gap'] >= min_gap and cond(h)]
    sel.sort(key=lambda h: h['lvl'])
    # corta os extremos (golpes acidentais / clipados) e divide em terços por nível
    if len(sel) >= 6:
        sel = sel[1:-1]
    groups = [g for g in np.array_split(np.arange(len(sel)), nlayers) if len(g)]
    xs = []
    for g in groups:
        # RR = os mais próximos da mediana de brilho do grupo (timbre consistente)
        grp = [sel[i] for i in g]
        med = np.median([h['bright'] for h in grp])
        grp.sort(key=lambda h: abs(h['bright'] - med))
        layer = []
        for h in grp[:max_rr]:
            end = h['a'] + int((min(h['gap'], max_sec) - 0.01) * SR)
            layer.append(trim(ts[h['a']:end], max_sec, fade=0.05, pre=0.004))
        xs.append((layer, [round(h['t'], 2) for h in grp[:max_rr]]))
    g = 0.89 / max(peak(x) for layer, _ in xs for x in layer)
    out = []
    for li, (layer, ts_) in enumerate(xs):
        out.append([save(f'{inst}-{zone}-{li}-{ri}', x * g) for ri, x in enumerate(layer)])
    add(inst, zone, out)
    report.append(f'{inst}.{zone}: TS1 {len(sel)} golpes → ' + ' | '.join(str(t) for _, t in xs))


ts_layers('shime', 'head', 78, 101, 0.42, 0.6)
ts_layers('okedo', 'head', 101, 120, 0.36, 0.9, cond=lambda h: h['bright'] < 1800)
ts_layers('okedo', 'rim', 101, 120, 0.24, 0.6, cond=lambda h: h['bright'] >= 1900, nlayers=1)

# nagadō/ō-nagadō/ōdaiko usam as baquetas do chū na borda
for inst in ('nagado', 'onagado', 'odaiko'):
    manifest[inst]['rim'] = manifest['chu']['rim']

# ---------- KANE: gongo pequeno de santuário (metal real) ----------
kn = load(os.path.join(SCR, 'fs/gong_kashiwa.mp3'))
strikes = [14.21, 26.80, 48.19, 59.46]
opens, damps = [], []
for s in strikes:
    a = int((s - 0.01) * SR)
    opens.append(trim(kn[a:a + int(1.8 * SR)], 1.8, fade=0.5, pre=0.003))
    damps.append(trim(kn[a:a + int(0.35 * SR)], 0.35, fade=0.2, pre=0.003))
g = 0.89 / max(peak(x) for x in opens)
manifest['kane'] = {
    'head': [[save(f'kane-head-0-{i}', x * g) for i, x in enumerate(opens)]],
    'rim': [[save(f'kane-rim-0-{i}', x * g) for i, x in enumerate(damps)]],
}
report.append('kane: KANE golpes ' + ', '.join(map(str, strikes)))

json.dump(manifest, open(os.path.join(OUT, 'manifest.json'), 'w'), indent=1)
print('\n'.join(report))
