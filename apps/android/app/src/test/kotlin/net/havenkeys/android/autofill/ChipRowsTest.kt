package net.havenkeys.android.autofill

import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.drawable.Icon
import android.util.Size
import android.view.View
import android.view.autofill.AutofillId
import android.view.autofill.AutofillValue
import android.view.inputmethod.InlineSuggestionsRequest
import android.widget.ImageView
import android.widget.inline.InlinePresentationSpec
import androidx.autofill.inline.UiVersions
import androidx.autofill.inline.v1.InlineSuggestionUi
import androidx.core.graphics.createBitmap
import net.havenkeys.android.ui.kit.HavenIcon
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ChipRowsTest {
    private val app = RuntimeEnvironment.getApplication()
    private val ids: List<AutofillId> = listOf(View(app).autofillId)
    private val auth = autofillSender(app, DatasetFactory.MODE_OPEN, null)

    private fun keyboard(max: Int): InlineSuggestionsRequest {
        val style = UiVersions.newStylesBuilder().addStyle(InlineSuggestionUi.newStyleBuilder().build()).build()
        val spec = InlinePresentationSpec.Builder(Size(0, 0), Size(Int.MAX_VALUE, Int.MAX_VALUE))
            .setStyle(style)
            .build()
        return InlineSuggestionsRequest.Builder(listOf(spec)).setMaxSuggestionCount(max).build()
    }

    @Test
    fun theIconIsPinnedOnlyInAKeyboardWithRoomForIt() {
        assertNull(Rows(app, null).pinnedDataset(ids, "Open HavenKeys", auth))
        val full = Rows(app, keyboard(1)).apply { inline("GitHub", null) }
        assertNull(full.pinnedDataset(ids, "Open HavenKeys", auth))
        assertNull(Rows(app, keyboard(1)).pinnedDataset(emptyList(), "Open HavenKeys", auth))
        assertNotNull(Rows(app, keyboard(1)).pinnedDataset(ids, "Open HavenKeys", auth))
    }

    @Test
    fun aRowKeptOutOfTheKeyboardStillShowsInTheDropdown() {
        val fields = ids.map { it to null as AutofillValue? }
        assertNotNull(Rows(app, keyboard(1)).dataset(fields, "Search HavenKeys…", null, auth, chip = false))
    }

    @Test
    fun everyChipPictureIsATileOfOneSize() {
        val icons = ChipIcons(app)
        val pictures = listOf(
            icons.monogram("GitHub"),
            icons.monogram(" "),
            icons.glyph(HavenIcon.IdCard),
            icons.card("visa"),
            icons.card("other"),
            icons.app(),
        )
        val sizes = pictures.map {
            assertEquals(Icon.TYPE_BITMAP, it.type)
            val drawable = it.loadDrawable(app)!!
            drawable.intrinsicWidth to drawable.intrinsicHeight
        }
        assertEquals(1, sizes.toSet().size)
    }

    // Gboard's chip style tints the start icon one flat colour; a picture keeps its own.
    @Test
    fun aKeyboardTintLeavesThePictureItsOwnColours() {
        val view = ImageView(app).apply {
            setImageIcon(ChipIcons(app).monogram("GitHub"))
            imageTintList = ColorStateList.valueOf(Color.WHITE)
        }
        val drawable = view.drawable
        val bitmap = createBitmap(drawable.intrinsicWidth, drawable.intrinsicHeight)
        drawable.setBounds(0, 0, bitmap.width, bitmap.height)
        drawable.draw(android.graphics.Canvas(bitmap))
        // A corner of the tile, clear of the letter: the avatar's dark ground.
        assertEquals(0xFF1D2B27.toInt(), bitmap.getPixel(bitmap.width / 2, bitmap.height / 8))
    }
}
