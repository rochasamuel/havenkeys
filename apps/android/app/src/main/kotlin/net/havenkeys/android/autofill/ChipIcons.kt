package net.havenkeys.android.autofill

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BlendMode
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.graphics.drawable.Icon
import android.os.Build
import androidx.core.content.ContextCompat
import androidx.core.content.res.ResourcesCompat
import androidx.core.graphics.PathParser
import androidx.core.graphics.createBitmap
import androidx.core.graphics.withSave
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.monogramOf
import kotlin.math.roundToInt

/**
 * The pictures at the start of keyboard chips, drawn as the app draws its
 * rows: a login's initial on the avatar tile, a card's network mark, the
 * identity's glyph, and HavenKeys' own round icon. Bitmaps that ignore the
 * keyboard's icon tint, so they keep their own colours. They name only what the chip's title
 * already shows.
 */
internal class ChipIcons(private val context: Context) {
    private val density = context.resources.displayMetrics.density
    private val size = (TILE_DP * density).roundToInt()

    /** The title's initial, as on the item's row; a blank title shows the key. */
    fun monogram(title: String): Icon = monogramOf(title)?.let { letter ->
        tile(AVATAR_BG) { canvas ->
            val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = AVATAR_FG
                textSize = size * MONOGRAM_RATIO
                textAlign = Paint.Align.CENTER
                typeface = ResourcesCompat.getFont(context, R.font.havenkeys_serif)
            }
            val baseline = size / 2f - (paint.descent() + paint.ascent()) / 2
            canvas.drawText(letter, size / 2f, baseline, paint)
        }
    } ?: glyph(HavenIcon.Key)

    /** A kind's glyph on the avatar tile (the identity, a card of no known network). */
    fun glyph(icon: HavenIcon): Icon = tile(AVATAR_BG) { canvas ->
        val path = PathParser.createPathFromPathData(icon.path)
        val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            color = AVATAR_FG
            strokeWidth = GLYPH_STROKE
            strokeCap = Paint.Cap.ROUND
            strokeJoin = Paint.Join.ROUND
        }
        canvas.withSave {
            // The glyph's 24-unit grid at half the tile, centred, as ItemTile draws it.
            val scale = size * GLYPH_SHARE / GLYPH_GRID
            val inset = (size - size * GLYPH_SHARE) / 2
            translate(inset, inset)
            scale(scale, scale)
            drawPath(path, paint)
        }
    }

    /** A card's network mark on a white tile; [brand] is the wire id (`"visa"`, …, `"other"`). */
    fun card(brand: String): Icon {
        val mark = BRANDS[brand] ?: return glyph(HavenIcon.Card)
        return tile(CARD_BG, border = CARD_LINE) { canvas ->
            val drawable = ContextCompat.getDrawable(context, mark) ?: return@tile
            val width = (size * MARK_WIDTH).roundToInt()
            val height = width * drawable.intrinsicHeight / drawable.intrinsicWidth
            val left = (size - width) / 2
            val top = (size - height) / 2
            drawable.setBounds(left, top, left + width, top + height)
            drawable.draw(canvas)
        }
    }

    /** HavenKeys' launcher icon, round: the visible middle of its adaptive layers. */
    fun app(): Icon {
        val bitmap = createBitmap(size, size)
        val canvas = Canvas(bitmap)
        val circle = Path().apply { addCircle(size / 2f, size / 2f, size / 2f, Path.Direction.CW) }
        canvas.clipPath(circle)
        // The adaptive canvas is 108 units, of which the middle 72 are visible.
        val bleed = (size * ADAPTIVE_BLEED).roundToInt()
        listOf(R.drawable.ic_launcher_background, R.drawable.ic_launcher_foreground).forEach { layer ->
            ContextCompat.getDrawable(context, layer)?.apply {
                setBounds(-bleed, -bleed, size + bleed, size + bleed)
                draw(canvas)
            }
        }
        return bitmap.icon()
    }

    private fun tile(background: Int, border: Int? = null, draw: (Canvas) -> Unit): Icon {
        val bitmap = createBitmap(size, size)
        val canvas = Canvas(bitmap)
        val radius = size * CORNER_SHARE
        val line = BORDER_DP * density
        val inset = if (border != null) line / 2 else 0f
        val shape = RectF(inset, inset, size - inset, size - inset)
        canvas.drawRoundRect(shape, radius, radius, Paint(Paint.ANTI_ALIAS_FLAG).apply { color = background })
        border?.let {
            val stroke = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                style = Paint.Style.STROKE
                color = it
                strokeWidth = line
            }
            canvas.drawRoundRect(shape, radius, radius, stroke)
        }
        draw(canvas)
        return bitmap.icon()
    }

    // Keyboards tint the start icon one colour; DST draws the picture as it is.
    // Chips need Android 11, so the guard only keeps lint's minimum honest.
    private fun Bitmap.icon(): Icon = Icon.createWithBitmap(this).apply {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) setTintBlendMode(BlendMode.DST)
    }

    private companion object {
        const val TILE_DP = 32
        const val MONOGRAM_RATIO = 0.5f
        const val GLYPH_GRID = 24f
        const val GLYPH_SHARE = 0.5f

        // ItemTile's corner: a quarter of the tile.
        const val CORNER_SHARE = 0.25f
        const val GLYPH_STROKE = 1.6f
        const val MARK_WIDTH = 0.86f
        const val BORDER_DP = 1f
        const val ADAPTIVE_BLEED = 18f / 72f

        // The light theme's avatar tile (Color.kt): a dark ground that reads on light and dark keyboards.
        const val AVATAR_BG = 0xFF1D2B27.toInt()
        const val AVATAR_FG = 0xFFE3C483.toInt()

        // The network marks are drawn for a white card.
        const val CARD_BG = 0xFFFFFFFF.toInt()
        const val CARD_LINE = 0xFFD5DBD8.toInt()

        val BRANDS: Map<String, Int> = mapOf(
            "visa" to R.drawable.ic_brand_visa,
            "mastercard" to R.drawable.ic_brand_mastercard,
            "amex" to R.drawable.ic_brand_amex,
            "elo" to R.drawable.ic_brand_elo,
            "hipercard" to R.drawable.ic_brand_hipercard,
            "diners" to R.drawable.ic_brand_diners,
            "discover" to R.drawable.ic_brand_discover,
            "jcb" to R.drawable.ic_brand_jcb,
            "unionpay" to R.drawable.ic_brand_unionpay,
            "maestro" to R.drawable.ic_brand_maestro,
        )
    }
}
