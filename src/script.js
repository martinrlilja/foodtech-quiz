import { h, Component, render } from 'https://unpkg.com/preact?module';
import htm from 'https://unpkg.com/htm?module';

const html = htm.bind(h);

export class QuizService {
  constructor(options) {
    this.userTokenKey = 'foodtech.userToken.v2';
    this.url = options.url;

    this.updatePoints();
  }

  setUserToken(token) {
    if (token) {
      window.localStorage.setItem(this.userTokenKey, token);
    }
  }

  getUserToken() {
    const token = window.localStorage.getItem(this.userTokenKey);
    if (token) {
      return `UserState ${token}`;
    } else {
      return null;
    }
  }

  async fetchWithUser(url, userToken, method, body) {
    const headers = new Headers();
    const options = {
      method: method || 'get',
    };

    if (userToken) {
      headers.append('Authorization', userToken);
    }

    if (body) {
      options.body = JSON.stringify(body);
      headers.append('Content-Type', 'application/json');
    }

    options.headers = headers;

    const response = await fetch(`${this.url}${url}`, options);
    return await response.json();
  }

  async updatePoints() {
    let points = 0;

    const userToken = this.getUserToken();
    if (userToken) {
      const data = await this.fetchWithUser('/stats', userToken);
      points = data.total_points;
    }

    const elements = document.querySelectorAll('.hamburger-count');
    for (const element of elements) {
      element.textContent = points;
    }
  }
}

export class Checkout {
  constructor(quizService) {
    this.quizService = quizService;
  }

  mount() {
    const forms = document.querySelectorAll('.checkout-form');

    for (const form of forms) {
      form.addEventListener('submit', async (event) => {
        event.preventDefault();
        const form_data = new FormData(form);
        const codes = form_data.get('codes')
          .split(' ')
          .map((code) => code.trim())
          .filter((code) => code.length > 0);
        const email = form_data.get('email');

        const userToken = this.quizService.getUserToken();
        const data = await this.quizService.fetchWithUser(
          '/checkout',
          userToken,
          'post',
          { codes: codes, email: email },
        );

        form.querySelector('.checkout-form__message').textContent =
          `You were registered with ${data.points} burgers! Good job!`;
      });
    }
  }
}

export class Quiz {
  constructor(quizService) {
    this.quizService = quizService;
  }

  mount(container) {
    const onClose = () => {
      render(html``, container);
    };

    window.addEventListener('click', async (event) => {
      const quizName = event.target.dataset.quizName;
      if (quizName) {
        const app = html`<${QuizPopup} quizName=${quizName} quizService=${this.quizService}
                                       onClose=${() => onClose()} />`;
        render(app, container);
      }
    });
  }
}

class QuizPopup extends Component {
  constructor() {
    super();
  }

  componentDidMount() {
    this.nextQuestion();
  }

  render(props) {
    if (this.state.isAlreadyCompleted) {
      return html`<${Popup}>
        <p class="text-lg font-bold text-gray-800">You completed the quiz!</p>
        <button onClick=${() => this.props.onClose()}
                class="block w-full mt-8 px-4 py-2 rounded-lg border border-red-500 bg-transparent hover:bg-red-500 text-red-500 hover:text-white font-semibold text-lg text-center">
          Close
        </button>
      </${Popup}>`;
    } else if (this.state.choices) {
      const choices = this.state.choices.map((choice) => {
        let colors = 'border-blue-500 bg-transparent text-blue-500 hover:bg-blue-500 hover:text-white';
        if (this.state.correctAnswers) {
          if (this.state.correctAnswers.includes(choice)) {
            colors = 'border-green-500 bg-green-500 text-white';
          } else if (this.state.selectedAnswer === choice) {
            colors = 'border-red-500 bg-red-500 text-white';
          }
        } else if (this.state.selectedAnswer === choice) {
          colors = 'border-blue-500 bg-blue-500 text-white';
        }

        const isEnabled = this.state.correctAnswers === null;

        return html`<li class="px-4 my-4">
          <button disabled=${!isEnabled} onClick=${() => this.onChoice(choice)}
                  class="block w-full px-2 py-1 rounded-lg border ${colors} font-semibold text-lg text-center">
            ${choice}
          </button>
        </li>`;
      });

      const hasSelectedAnswer = this.state.selectedAnswer !== null;
      let continueColors = 'border-green-500 bg-transparent hover:bg-green-500 text-green-500 hover:text-white';
      if (!hasSelectedAnswer) {
        continueColors = 'border-gray-500 text-gray-500';
      }

      return html`<${Popup}>
        ${this.state.isCorrect === true && html`<p class="font-bold text-green-600">Correct!</p>`}
        ${this.state.isCorrect === false && html`<p class="font-bold text-red-600">Incorrect</p>`}
        ${this.state.isCorrect === null && html`<p class="text-gray-600">Question</p>`}
        <p class="text-lg font-bold text-gray-800 mb-8">${this.state.question}</p>
        <ul>
          ${choices}
        </ul>
        <button disabled=${!hasSelectedAnswer} onClick=${() => this.onContinue()}
                class="block w-full mt-8 px-4 py-2 rounded-lg border ${continueColors} font-semibold text-lg text-center">
          ${!this.state.correctAnswers && 'Continue'}
          ${!!this.state.correctAnswers && 'Next question'}
        </button>
      </${Popup}>`;
    } else {
      return html`<${Popup}><p class="text-lg font-bold text-gray-800">Loading…</p></${Popup}>`;
    }
  }

  onChoice(answer) {
    this.setState(Object.assign(this.state, { selectedAnswer: answer }));
  }

  async onContinue() {
    if (!this.state.correctAnswers) {
      const userToken = this.props.quizService.getUserToken();
      const data = await this.props.quizService.fetchWithUser(
        `/quiz/${this.props.quizName}`,
        userToken,
        'post',
        { answer: this.state.selectedAnswer },
      );

      this.setState(
        Object.assign(
          this.state,
          { isCorrect: data.is_correct, correctAnswers: data.correct },
        )
      );

      this.props.quizService.setUserToken(data.token);
      await this.props.quizService.updatePoints();
    } else {
      await this.nextQuestion();
    }
  }

  async nextQuestion() {
    const userToken = this.props.quizService.getUserToken();
    const data = await this.props.quizService.fetchWithUser(`/quiz/${this.props.quizName}`, userToken);

    if (data.error === 'NotFound') {
      this.setState({ isAlreadyCompleted: true });
    } else {
      this.props.quizService.setUserToken(data.token);
      this.setState({
        question: data.question,
        choices: data.choices,
        selectedAnswer: null,
        correctAnswers: null,
        isCorrect: null,
      });
    }
  }
}

function Popup(props) {
  return html`<div class="fixed inset-0 flex justify-center items-center" style="background: rgba(0, 0, 0, 0.3)">
    <div class="bg-white shadow-2xl p-6 md:p-12 m-4 rounded-lg" style="min-width: 280px; max-width: 40rem;">
      ${props.children}
    </div>
  </div>`;
}

export class Wheel {
  constructor(quizService) {
    this.quizService = quizService;
    this.images = {};

    const wheels = document.querySelectorAll('.wheel');
    for (const canvas of wheels) {
      canvas.width = 600;
      canvas.height = 600;

      const imageSource = canvas.dataset.image;
      const image = document.createElement('img');
      image.src = imageSource;
      image.addEventListener('load', () => {
        const context = canvas.getContext('2d');
        this.renderWheel(imageSource, context, 0);
      });

      this.images[imageSource] = image;
    }
  }

  mount() {
    window.addEventListener('click', async (event) => {
      const wheelName = event.target.dataset.wheelName;
      const canvas = document.querySelector('.wheel');

      if (wheelName) {
        await this.spinWheel(wheelName, canvas);
      }
    });
  }

  renderWheel(image, context, angle) {
    const width = 600;
    const height = 600;
    context.clearRect(0, 0, width, height);

    context.lineWidth = 2;

    context.translate(width / 2, height / 2);

    context.beginPath();
    context.moveTo(-15, -height * 0.4 - 15);
    context.lineTo(15, -height * 0.4 - 15);
    context.lineTo(0, -height * 0.4 + 10);
    context.fill();

    context.rotate(angle);

    context.beginPath();
    context.arc(0, 0, width * 0.4, 0, 2 * Math.PI);

    const spokeAngles = [0, Math.PI / 3, Math.PI / 3 * 2];
    for (const spokeAngle of spokeAngles) {
      const a = spokeAngle;

      context.moveTo(
        width * 0.4 * Math.cos(a),
        height * 0.4 * Math.sin(a)
      );

      context.lineTo(
        -width * 0.4 * Math.cos(a),
        -height * 0.4 * Math.sin(a)
      );
    }

    context.stroke();

    if (this.images[image]) {
      context.drawImage(this.images[image], -40, -height * 0.4 + 20, 80, 80);
    }

    context.font = '32px system-ui, sans-serif';

    const renderText = (text, textAngles) => {
      const textSize = context.measureText(text);
      for (const textAngle of textAngles) {
        context.setTransform(1, 0, 0, 1, 0, 0);
        context.translate(width / 2, height / 2);
        context.rotate(angle + textAngle);

        context.fillText(
          text,
          -textSize.width / 2,
          -height * 0.4 + 50
        );
      }
    };

    renderText('20', [Math.PI / 3, Math.PI, Math.PI / 3 * 5]);
    renderText('40', [Math.PI / 3 * 2, Math.PI / 3 * 4]);

    context.setTransform(1, 0, 0, 1, 0, 0);
  }

  async spinWheel(name, canvas) {
    const userToken = this.quizService.getUserToken();
    const data = await this.quizService.fetchWithUser(
      `/wheel/${name}`,
      userToken,
      'post',
      { },
    );

    if (data.error === 'NotFound') {
      alert('You already spun this wheel.');
    } else {
      const context = canvas.getContext('2d');

      const angles = {
        20: [Math.PI / 3, Math.PI, Math.PI / 3 * 5],
        40: [Math.PI / 3 * 2, Math.PI / 3 * 4],
        60: [0],
      };
      const pointAngles = angles[data.points];
      const pointAngle = pointAngles[Math.floor(Math.random() * pointAngles.length)];
      const variation = Math.random() * Math.PI / 3.1 - Math.PI / 6.2;

      const extraLaps = Math.round(Math.random() * 2) + 3;
      const targetAngle = pointAngle + extraLaps * Math.PI * 2 + variation;

      this.renderSpinningWheel(canvas.dataset.image, context, targetAngle, null);

      this.quizService.setUserToken(data.token);
      await this.quizService.updatePoints();
    }
  }

  renderSpinningWheel(image, context, targetAngle, startTimestamp) {
    window.requestAnimationFrame((timestamp) => {
      if (startTimestamp === null) {
        startTimestamp = timestamp;
      }
      const delta = timestamp - startTimestamp;
      const clampedDelta = Math.min(1, delta / 5e3);
      //const smooth = this.cubicBezier(0.7, 0.95, clampedDelta);
      const smooth = this.cosine(clampedDelta);

      this.renderWheel(image, context, smooth * targetAngle);

      if (clampedDelta < 1) {
        this.renderSpinningWheel(image, context, targetAngle, startTimestamp);
      }
    });
  }

  cosine(t) {
    // http://paulbourke.net/miscellaneous/interpolation/
    return (1 - Math.cos(t * Math.PI)) / 2;
  }

  cubicBezier(p1, p2, t) {
    return 3 * Math.pow(1 - t, 2) * t * p1
      + 3 * (1 - t) * Math.pow(t, 2) * p2
      + Math.pow(t, 3);
  }
}
