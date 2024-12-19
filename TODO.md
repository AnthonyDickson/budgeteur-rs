# To Do

- Implement session store.
  - Sessions are intended to solve issues with using cookies on their own.
    When using cookies for token like auth, you cannot control the cookie
    expiry freely. It is easy to set the cookie expiry five minutes into the
    future, but you cannot check the expiry of a cookie sent by the client
    since the client will only send the cookie key values and not the metadata.

    An alternative approach would be to add a key value pair to the cookie jar
    that encodes the session expiry. This would allow for the server to finely
    control the cookie/session expiry without requiring an extra database.
    Ensuring that the cookie is signed and encrypted as well as HTTP only will
    prevent the cookie from being tampered with.
  - A session should have the below data members:
    - User ID
    - Expiry
  - The values should be set as follows:
    - User ID should be set to the user ID of the user who just logged in via
      the log in route.
    - Expiry should be initially set to:
      - 5 minutes OR
      - one week if the user selects "remember me" at the log in screen.
    - Session expiry should be extended to 5 minutes past the current time if
      the session would expire within 5 minutes.
    - Sessions should be destroyed when the user logs out.
