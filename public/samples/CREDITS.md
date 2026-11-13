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
outras alterações: 26 letras (A–Z) de 0,15 s cada, em sequência. Usado nas
vozes dos kiai (kakegoe), recortado letra a letra e transposto em tempo real.

## Hira-daiko pequeno — derivado (CC BY-SA 4.0)

`hira-head-<camada>-<variação>.ogg` são derivados de `chu-head-*` (SCC Taiko
Drums, acima), então seguem a licença **CC BY-SA 4.0**. Processamento (ffmpeg):
transposição de +4,6 / +5 / +5,4 semitons (uma por variação), highpass 110 Hz,
+4 dB em 2,6 kHz (estalo da baqueta), −3 dB em 320 Hz e corpo encurtado
(fade a partir de 0,35 s, 0,8 s no total) — som de um tambor raso e pequeno.
A borda usa os mesmos `chu-rim-*`, um pouco mais aguda.
