package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Test

class FieldGroupsTest {
    private fun input(form: Int? = null, vararg ancestors: Int, text: Boolean = true) =
        GroupInput(form, ancestors.toList(), text)

    @Test
    fun aFieldInAFormBelongsToTheForm() {
        val groups = FieldGroups.assign(listOf(input(form = 7, 3, 2, 1), input(form = 7, 4, 2, 1)))
        assertEquals(listOf(htmlForm(7), htmlForm(7)), groups)
    }

    @Test
    fun aPageWideFormIsNoGroupAndTheFieldClimbsInstead() {
        // ASP.NET wraps the page in one form: 41 fields are no login form.
        val inputs = List(FieldGroups.MAX_FORM_INPUTS + 1) { input(form = 1, 100 + it / 2, 1) }
        val groups = FieldGroups.assign(inputs)
        assertEquals(viewGroup(100), groups[0])
        assertEquals(viewGroup(100), groups[1])
    }

    @Test
    fun aViewGroupIsTheNearestContainerWithAnotherTextField() {
        // LinearLayout(10) > [TextInputLayout(11) > FrameLayout(12) > email],
        //                    [TextInputLayout(13) > FrameLayout(14) > password]
        val email = input(null, 12, 11, 10, 1)
        val password = input(null, 14, 13, 10, 1)
        assertEquals(listOf(viewGroup(10), viewGroup(10)), FieldGroups.assign(listOf(email, password)))
    }

    @Test
    fun aListDoesNotMakeAGroup() {
        // A country list beside the phone box: the phone climbs on to the password.
        val country = input(null, 20, 10, text = false)
        val phone = input(null, 20, 10)
        val password = input(null, 21, 10)
        assertEquals(viewGroup(10), FieldGroups.assign(listOf(country, phone, password))[1])
    }

    @Test
    fun aFieldAloneUnderTheTopHasNoGroup() {
        assertNull(FieldGroups.assign(listOf(input(null))).single())
        // More than a form's worth of fields under the first parent: the whole screen.
        val crowd = List(FieldGroups.MAX_FORM_INPUTS + 1) { input(null, 1) }
        assertNull(FieldGroups.assign(crowd).first())
    }

    @Test
    fun theClimbStopsAfterMaxClimbLevels() {
        val deep = (1..20).toList().toIntArray()
        val lone = FieldGroups.assign(listOf(input(null, *deep), input(null, 20)))
        assertEquals(viewGroup(FieldGroups.MAX_CLIMB), lone[0])
    }

    @Test
    fun nothingFocusedKeepsEveryField() {
        val fields = listOf(
            field(inputType = email, group = htmlForm(1)),
            field(inputType = password, group = htmlForm(2)),
        )
        assertSame(fields, FieldGroups.focusedGroup(fields))
    }

    @Test
    fun aFocusedFieldWithoutAGroupKeepsEveryField() {
        val fields = listOf(field(inputType = email, focused = true), field(inputType = password, group = htmlForm(2)))
        assertSame(fields, FieldGroups.focusedGroup(fields))
    }

    @Test
    fun aFocusedFormKeepsOnlyItsFields() {
        val user = field(inputType = email, group = htmlForm(1))
        val pw = field(inputType = password, group = htmlForm(1))
        val news = field(inputType = email, focused = true, group = htmlForm(2))
        assertEquals(listOf(news), FieldGroups.focusedGroup(listOf(user, pw, news)))
        val focusedUser = user.copy(focused = true)
        val fields = listOf(focusedUser, pw, news.copy(focused = false))
        assertEquals(listOf(focusedUser, pw), FieldGroups.focusedGroup(fields))
    }

    @Test
    fun aViewGroupWithItsOwnPasswordIsTheGroup() {
        val user = field(inputType = email, focused = true, group = viewGroup(1))
        val pw = field(inputType = password, group = viewGroup(1))
        val search = field(id = "search", group = viewGroup(2))
        assertEquals(listOf(user, pw), FieldGroups.focusedGroup(listOf(search, user, pw)))
    }

    @Test
    fun aViewGroupThatWouldLoseThePasswordIsNotUsed() {
        // A row of country code and phone, the password below: the whole window, as before groups.
        val code = field(id = "country_code", group = viewGroup(1))
        val phone = field(hints = listOf("phone"), focused = true, group = viewGroup(1))
        val pw = field(inputType = password, group = viewGroup(2))
        val fields = listOf(code, phone, pw)
        assertSame(fields, FieldGroups.focusedGroup(fields))
    }

    @Test
    fun aViewGroupOnAWindowWithoutASignInIsTheGroup() {
        val news = field(inputType = email, focused = true, group = viewGroup(1))
        val name = field(hint = "Name", group = viewGroup(1))
        val search = field(id = "search", group = viewGroup(2))
        assertEquals(listOf(news, name), FieldGroups.focusedGroup(listOf(news, name, search)))
    }

    @Test
    fun thousandsOfGroupedFieldsRouteFast() {
        // CLAUDE.md attack 7, with the focused field in a view group of its own.
        assertScalesLinearly(
            page = { n ->
                List(n) {
                    val group = if (it % 3 == 0) htmlForm(it / 30) else viewGroup(it / 9)
                    when (it % 4) {
                        0 -> field(inputType = email, group = group, focused = it == 4)
                        1 -> field(inputType = password, group = group)
                        else -> field(hint = "Name", group = group)
                    }
                }
            },
            work = { FormRouter.route(it) },
        )
    }
}
