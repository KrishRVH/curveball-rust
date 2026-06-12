tellTarget(enemyLives)
{
   gotoAndPlay("L" + _parent.world.enemyLives);
}
tellTarget(playerLives)
{
   gotoAndPlay("L" + _parent.world.playerLives);
}
world.speed = levelSpeed[world.level - 1];
world.skillFactor = levelSkillFactor[world.level - 1];
world.curveAmount = levelCurve[world.level - 1];
world.hitScore = 100;
world.curveBonus = 50;
world.superCurveBonus = 150;
world.accuracyBonus = 100;
world.bonusDisplay = 3000;
bonusScore = world.bonusDisplay;
world.bonus = 10;
