if(_parent.world.enemyLives < 1)
{
   tellTarget(_parent)
   {
      world.level += 1;
      level = "Level " + world.level;
      levelNumber = world.level;
      world.enemyLives = 3;
      world.score += world.bonusDisplay;
      score = world.score;
      gotoAndStop("Level");
      play();
   }
}
else if(_parent.world.playerLives < 1)
{
   tellTarget(_parent)
   {
      bonusScore = "";
      bonusWord = "";
      gotoAndStop("GameOver");
      play();
   }
}
else
{
   tellTarget(_parent)
   {
      gotoAndStop("Serve");
      play();
   }
}
