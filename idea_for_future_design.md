How to implement saving accounts?
- Accounts can predict income...
    - ... that happens at a specific point in the year
    - ... that is based on the percentage of the amount in the account
- We don't attribute specific income to specific goals, instead all
  income goes to feed a global predicted amount in the pool. That
  amount is then spread to goals
- The remaining computation for the current period does not change,
  instead there are just month where more money remains (ie. a little bit
  of the saving accounts revenue are placed each period).
  - DRAWBACK -> that might be a bit tough, as apparently, there are
    scenarios where one can make more than a thousand euros a year
  - The goals computation might need to get a bit more complicated if I
    realize that I'm left with close to nothing every months except at
    the end of the year where I get a 1000 euros
    - If the computation get more complicated, I will need a check at
      the start of the app the verifies whether the goals can be
      achieved with the current predicted income
- A new screen is added to the app where one can see:
  - ... the predicted accumutation of money in the vault
  - ... what percentage of the money is assigned to goals
  - QUESTION -> how to predict non-goals living expenses which will go
    toward reducing the goals?
