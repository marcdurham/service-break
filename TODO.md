# TODO
- 108 Make places editable for users who are logged in, log to the database changes made, when, and by whom.
- 113 Change the top filters Shop, Store, Mall, Park, Public, Hall to a single chip that can be tapped to show things options called "Type" (mean type of place) instead add the things that are offered like Restroom, Coffee, Food, Seating a the top level, by default show them all.
- 120 Add an admin account, with the default password of "I brake for coffee", which will be in the documenation, that has an admin menu which includes a way to back-up all data and download it as a single JSON file, but not passwords, store newly generated passwords.  Also include a way to import all data on that same page so that the data can quickly be exported, the app re-deployed and the data can quickly be restored.
- 117 Add a back-end bash script to reset user passwords like ./scripts/change-password.sh {user} {password} where if the password is omitted you are prompted for it.
- 100 Add a password confirm for registering new accounts
- 101 Only allow invited users to register, include a one-time invitation code that expires after 7 days, put the code in the URI so that an easy link can be shared, only allow existing users to invite new users. Only allow 5 invitations per day to be sent by each user, but new usrs have to wait only after 24 hours of registering to send invitations. 
- 102 Add a 'Name' field to invitations, that the invited user can change once they've registered, Show a list of invitations and friends and their status on the account menu.
- 103 Move the 'Invite' menu item off the menu and make it a button on the Account page.
- 104 Don't put 'BREAK-' in the invite code, just have the code part of the code.
- 105 Make a new page for creating accounts that can be navigated to from the account page, that's where the invite code would be entered, or pre-entered if it was already in the URI from the invitation link.  The invitation link would take a new user directly to th new "create an account" page.  
- 106 Change this phrase "Invite your fellow travelers" to "Invite your friends".
- 109 Make the ratings specific to aspects like bathroom cleanliness, coffee, or food.
- 116 re-generate the invite code once copy or share it used.  Add the text "One time use code"

# BACKLOG (Don't do the tasks below yet)
- 118 On the "Nearby places" page automatically expand the range until at least one place appears or you reach the max limit.
- 119 When opening the "Nearby places" from the "Map" page show the same places on the List that were on the map.  Show a compact count of how many places are being shown on the map view.
- 110 Make a 'going there' button for places, with a modal menu that includes being able to share a link to where you're going and selecting a time (When) you think you'll arrive, only allow 24 hours in advance so the user who is sharing doesn't need to pick a date, just a time.
- 115 ?Add a feature that shows your friend's friends
- 111 Add a "Share location" option to share with your friends, but only allow it to share for 8 hours.  Include when the last time your location was updated.
- 114 Is there a way to get a list of business with addresses and name near a geolocation?
