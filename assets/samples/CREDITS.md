# Créditos dos samples

Todos os `.ogg` deste diretório são estéreo, Ogg Vorbis q6, cortados em golpes
individuais e normalizados por instrumento (a diferença de força entre as
camadas foi preservada). Nomes: `<instrumento>-<zona>-<camada>-<variação>.ogg`,
com zona `head` = couro e `rim` = borda; camada 0 = mais suave.

## SCC Taiko Drums — CC BY-SA 4.0

https://github.com/floe-audio/Taiko-Drums, gravações de S. Christian Collins e
Subaqueous, licença **CC BY-SA 4.0** (https://creativecommons.org/licenses/by-sa/4.0/).
Camadas e variações seguem o mapeamento do SFZ original.

| Aqui | Original |
|---|---|
| chu-head-* | taiko1-p, taiko1-m / taiko1-f1, taiko1-f2 |
| chu-rim-* (borda de chū, nagadō, ō-nagadō, ōdaiko) | Taiko Drum Sticks-1,0 / 2,3,5 / 4,6 |
| nagado-head-* | Taiko Drum Hit 1-15, 1-16 / 1-27, 1-28, 2-6 |
| onagado-head-* | Taiko Drum Hit 2-2, 1-25 / 1-18, 2-7 / 2-5 |
| odaiko-head-* | Taiko Drum Hit 3-4, 3-3 / 3-2 / 3-1 / 3-0 |
| shime-rim-* | Taiko Lt Sticks-0, 1 / 2 / 3, 4 |

Como são derivados de material CC BY-SA, estes arquivos continuam sob a mesma licença.

## taikoset1_ste58 — CC BY 3.0

"taikoset1_ste58.wav" por **whatsanickname4u** (baterista Dave Thayer),
https://freesound.org/people/whatsanickname4u/sounds/127337/, licença
**CC BY 3.0** (https://creativecommons.org/licenses/by/3.0/). Gravação de taikos
reais numa sala comunitária (Zoom H2, 2009).

| Aqui | Trecho do original |
|---|---|
| shime-head-* | golpes isolados do shime-daiko (78–101 s) |
| okedo-head-*, okedo-rim-* | golpes isolados do okedō-daiko (101–120 s), borda = golpes mais brilhantes |

## Gongo pequeno de santuário — CC0

"Japan_Kashiwa_Shrine_Hitting_a_Small_Gong.wav" por **RutgerMuller**,
https://freesound.org/people/RutgerMuller/sounds/364847/, domínio público (CC0).
`kane-head-*` = golpe aberto (1,8 s); `kane-rim-*` = o mesmo golpe abafado (0,35 s).

## Animalese — CC BY 4.0

`animalese.ogg` = "animalese.wav" do **animalese.js** de **Josh Simmons**
(https://github.com/acedio/animalese.js), licença **CC BY 4.0**
(https://creativecommons.org/licenses/by/4.0/). Convertido para Ogg Vorbis sem
outras alterações: 26 letras (A–Z) de 0,15 s cada, em sequência. Usado na
voz padrão dos kiai (kakegoe), recortado letra a letra e transposto em tempo real,
com timbre de grito.

## Hira-daiko pequeno — derivado (CC BY-SA 4.0)

`hira-head-<camada>-<variação>.ogg` são derivados de `chu-head-*` (SCC Taiko
Drums, acima), então seguem a licença **CC BY-SA 4.0**. Processamento (ffmpeg):
transposição de +4,6 / +5 / +5,4 semitons (uma por variação), highpass 110 Hz,
+4 dB em 2,6 kHz (estalo da baqueta), −3 dB em 320 Hz e corpo encurtado
(fade a partir de 0,35 s, 0,8 s no total) — som de um tambor raso e pequeno.
A borda usa os mesmos `chu-rim-*`, um pouco mais aguda.

## Kiai (kakegoe) e contagem — CC0 / CC BY 3.0 / CC BY 4.0

Gravações reais de vozes em `kiai/`, usadas nos kiai (vozes masc/fem, opcionais) e na contagem falada do count-in.
Todas do Freesound (prévias HQ), recortadas na palavra (≈10 ms de folga antes,
fade-out de 15 ms; até 150–200 ms nos sons longos truncados em 1,2 s),
passa-altas de 70 Hz, pico normalizado em −1 dBFS, 48 kHz, estéreo (mono
duplicado), Ogg Vorbis q6. Nomes: `<voz>-<palavra>-<n>.ogg` (gritos) e
`<voz>-count-<número>-<n>.ogg` (contagem), voz `m`/`f`. A lista completa, com
fonte, autor, licença e trecho exato de cada arquivo, está em `kiai/manifest.json`.
Licenças: CC0 = https://creativecommons.org/publicdomain/zero/1.0/,
CC BY 3.0 = https://creativecommons.org/licenses/by/3.0/,
CC BY 4.0 = https://creativecommons.org/licenses/by/4.0/.

| Aqui | Original (autor, licença) | Trecho |
|---|---|---|
| m-ha-0 | [“"HA!" - NO reverb.wav”](https://freesound.org/people/WelvynZPorterSamples/sounds/623441/) por **WelvynZPorterSamples**, CC0 1.0 | 0.14–0.65 s |
| m-ha-1, m-ha-2 | [“Cartoon-like male attack effort vocal sfx”](https://freesound.org/people/A_kn/sounds/712116/) por **A_kn**, CC0 1.0 | 4.59–4.87 s; 7.93–8.27 s |
| m-ha-3, m-hai-1, m-orya-0 | [“Ninja Vocalizations, Several Types.wav”](https://freesound.org/people/WannyManny/sounds/632075/) por **WannyManny**, CC0 1.0 | 3.46–3.71 s; 10.64–11.17 s; 24.07–25.27 s |
| m-hai-0 | [“Japanese "hai"”](https://freesound.org/people/Vilaa2404/sounds/867892/) por **Vilaa2404**, CC BY 4.0 | 0.00–0.60 s |
| m-yo-0 | [“Japanese yo 2short”](https://freesound.org/people/Vilaa2404/sounds/867896/) por **Vilaa2404**, CC BY 4.0 | 0.00–1.20 s |
| m-yo-1 | [“Japanese yo 3short”](https://freesound.org/people/Vilaa2404/sounds/867898/) por **Vilaa2404**, CC BY 4.0 | 0.00–1.20 s |
| m-yo-2 | [“Japanese "yoooooo" 4”](https://freesound.org/people/Vilaa2404/sounds/867899/) por **Vilaa2404**, CC BY 4.0 | 0.03–1.20 s |
| m-iyo-0 | [“Japanese yo 5short”](https://freesound.org/people/Vilaa2404/sounds/867901/) por **Vilaa2404**, CC BY 4.0 | 0.00–1.20 s |
| m-iyo-1 | [“Japanese yo 4short”](https://freesound.org/people/Vilaa2404/sounds/867900/) por **Vilaa2404**, CC BY 4.0 | 0.00–1.20 s |
| m-iya-0, m-iya-1 | [“Kung Fu Yell.aif”](https://freesound.org/people/oldedgar/sounds/97977/) por **oldedgar**, CC0 1.0 | 2.05–3.25 s; 0.26–1.20 s |
| m-haiya-0 | [“GritoNinja.wav”](https://freesound.org/people/Envywolf/sounds/392762/) por **Envywolf**, CC BY 4.0 | 0.88–1.61 s |
| m-ya-0 | [“Voice_Male_Attack_Mono.wav”](https://freesound.org/people/Nox_Sound/sounds/474651/) por **Nox_Sound**, CC0 1.0 | 0.12–0.66 s |
| m-ei-0 | [“Hey - Men Shouting together - Jewish Wedding”](https://freesound.org/people/khenshom/sounds/527740/) por **khenshom**, CC0 1.0 | 0.03–0.51 s |
| f-hai-0 | [“japanese - yes1.wav”](https://freesound.org/people/Reitanna/sounds/235015/) por **Reitanna**, CC0 1.0 | 0.06–0.38 s |
| f-hai-1 | [“japanese - yes2.wav”](https://freesound.org/people/Reitanna/sounds/235118/) por **Reitanna**, CC0 1.0 | 0.08–0.39 s |
| f-hai-2 | [“japanese - yes4.wav”](https://freesound.org/people/Reitanna/sounds/235012/) por **Reitanna**, CC0 1.0 | 0.05–0.38 s |
| f-hai-3 | [“hai/yes 2”](https://freesound.org/people/Alivvie/sounds/323194/) por **Alivvie**, CC0 1.0 | 0.03–0.62 s |
| f-ha-0 | [“Female Haha Sound 2 (A)”](https://freesound.org/people/vibritherabjit123/sounds/721133/) por **vibritherabjit123**, CC BY 4.0 | 0.06–0.31 s |
| f-ha-1 | [“Female Haha Sound 1 (A)”](https://freesound.org/people/vibritherabjit123/sounds/721132/) por **vibritherabjit123**, CC BY 4.0 | 0.11–0.37 s |
| f-ha-2, f-ha-3, f-iya-1 | [“Female Grunts For Games”](https://freesound.org/people/SkyRaeVoicing/sounds/368843/) por **SkyRaeVoicing**, CC BY 3.0 | 0.29–0.52 s; 1.04–1.28 s; 1.72–2.09 s |
| f-ha-4, f-ho-0 | [“game_vocalizations.wav”](https://freesound.org/people/montblanccandies/sounds/273200/) por **montblanccandies**, CC BY 3.0 | 17.30–17.56 s; 16.63–16.86 s |
| f-sa-0 | [“Fighting Game grunts - young female.wav”](https://freesound.org/people/AderuMoro/sounds/213295/) por **AderuMoro**, CC BY 4.0 | 0.38–1.03 s |
| f-iya-0 | [“iya2.wav”](https://freesound.org/people/montblanccandies/sounds/271598/) por **montblanccandies**, CC BY 3.0 | 0.71–1.35 s |
| f-hya-0 | [“Female Hyah Sound 1 (A)”](https://freesound.org/people/vibritherabjit123/sounds/721123/) por **vibritherabjit123**, CC BY 4.0 | 0.03–0.71 s |
| f-hyo-0 | [“Female Hyoh Sound (A)”](https://freesound.org/people/vibritherabjit123/sounds/721137/) por **vibritherabjit123**, CC BY 4.0 | 0.03–0.44 s |
| f-ya-0 | [“yatta”](https://freesound.org/people/Alivvie/sounds/323202/) por **Alivvie**, CC0 1.0 | 0.08–0.44 s |
| m-count-ichi-0, m-count-ni-0, m-count-san-0, m-count-yon-0, m-count-go-0, m-count-roku-0, m-count-nana-0, m-count-hachi-0 | [“japanese.numbers.wav”](https://freesound.org/people/dobroide/sounds/52786/) por **dobroide**, CC BY 4.0 | 0.13–0.60 s; 1.05–1.50 s; 2.29–2.87 s; 3.59–4.09 s; 4.79–5.28 s; 5.99–6.65 s; 7.18–7.71 s; 8.35–8.97 s |
| f-count-ichi-0, f-count-ni-0, f-count-san-0, f-count-shi-0, f-count-go-0, f-count-roku-0, f-count-shichi-0, f-count-hachi-0 | [“counting 0-10”](https://freesound.org/people/Alivvie/sounds/323188/) por **Alivvie**, CC0 1.0 | 0.91–1.40 s; 1.52–2.06 s; 2.38–2.90 s; 3.03–3.41 s; 3.77–4.19 s; 4.49–4.87 s; 5.33–5.84 s; 6.11–6.52 s |

