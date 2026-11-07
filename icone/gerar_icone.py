"""Gera icone/aula.ico (e aula.png de prévia): slide com fita de marcador.

Uso: python3 icone/gerar_icone.py   (precisa de Pillow)
"""
from pathlib import Path

from PIL import Image, ImageDraw

AQUI = Path(__file__).resolve().parent
S = 1024  # desenha grande e reduz (antisserrilhado)


def gradiente(w, h, c1, c2):
    g = Image.new("RGB", (1, h))
    for y in range(h):
        t = y / (h - 1)
        g.putpixel((0, y), tuple(round(a + (b - a) * t) for a, b in zip(c1, c2)))
    return g.resize((w, h))


def desenhar(detalhe=True):
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    # fundo: quadrado arredondado azul
    mascara = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mascara).rounded_rectangle([40, 40, S - 40, S - 40], radius=200, fill=255)
    img.paste(gradiente(S, S, (37, 99, 235), (30, 58, 138)), (0, 0), mascara)

    d = ImageDraw.Draw(img)
    # sombra do slide
    d.rounded_rectangle([196, 276, 868, 760], radius=48, fill=(15, 30, 80, 110))
    # slide
    x0, y0, x1, y1 = 172, 244, 852, 728
    d.rounded_rectangle([x0, y0, x1, y1], radius=48, fill=(255, 255, 255))
    if detalhe:
        # título e linhas de texto
        d.rounded_rectangle([240, 320, 600, 372], radius=26, fill=(30, 58, 138))
        for i, w in enumerate([520, 460, 380]):
            y = 430 + i * 72
            d.rounded_rectangle([240, y, 240 + w, y + 34], radius=17, fill=(191, 203, 222))
    else:
        d.rounded_rectangle([240, 330, 640, 420], radius=45, fill=(30, 58, 138))
        d.rounded_rectangle([240, 500, 560, 580], radius=40, fill=(191, 203, 222))
    # fita de marcador laranja (onde parou)
    fx0, fx1, ftop, fbot = 650, 790, 196, 560
    d.polygon(
        [(fx0, ftop), (fx1, ftop), (fx1, fbot), ((fx0 + fx1) / 2, fbot - 70), (fx0, fbot)],
        fill=(249, 115, 22),
    )
    d.rectangle([fx0, ftop, fx1, ftop + 26], fill=(234, 88, 12))
    # pé do projetor/tela
    d.rounded_rectangle([482, 728, 542, 840], radius=12, fill=(255, 255, 255))
    d.rounded_rectangle([372, 820, 652, 868], radius=24, fill=(255, 255, 255))
    return img


def main():
    grande = desenhar(True)
    simples = desenhar(False)  # menos detalhe para tamanhos pequenos
    tamanhos = [16, 20, 24, 32, 40, 48, 64, 128, 256]
    imgs = [
        (simples if t <= 32 else grande).resize((t, t), Image.LANCZOS) for t in tamanhos
    ]
    imgs[-1].save(AQUI / "aula.ico", sizes=[(t, t) for t in tamanhos], append_images=imgs[:-1])
    grande.resize((256, 256), Image.LANCZOS).save(AQUI / "aula.png")
    # folha de prévia: todos os tamanhos lado a lado
    folha = Image.new("RGBA", (sum(tamanhos) + 10 * len(tamanhos), 256), (240, 240, 240, 255))
    x = 0
    for t, im in zip(tamanhos, imgs):
        folha.paste(im, (x, 256 - t), im)
        x += t + 10
    folha.save(AQUI / "previa.png")


if __name__ == "__main__":
    main()
